//! Typed event emission and transactional deferred delivery.

use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc, time::Duration};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{sync::Notify, task::JoinHandle};

use crate::models::events;

tokio::task_local! {
    static INLINE_DISPATCH: ();
}

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);
const BATCH_SIZE: u64 = 100;
const MAX_ERROR_LENGTH: usize = 1_024;

/// A compile-time checked event payload.
///
/// Events stay typed at the write site and become JSON only after serialization
/// succeeds inside an [`EventTransaction`].
pub trait Event: Serialize + Send + Sync {
    const NAME: &'static str;
}

/// Persisted representation consumed by subscribers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub id: i64,
    pub name: String,
    pub payload: Value,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
}

impl From<events::Model> for EventEnvelope {
    fn from(event: events::Model) -> Self {
        Self {
            id: event.id,
            name: event.name,
            payload: event.payload,
            created_at: event.created_at,
        }
    }
}

/// Whether a subscriber runs before or after the action commits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriberMode {
    /// Runs inside the action transaction. Failure aborts the action.
    Inline,
    /// Runs from the persisted outbox only after the action commits.
    Deferred,
}

/// Future returned by an inline subscriber.
pub type InlineSubscriberFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

type InlineHandler = Arc<
    dyn for<'a> Fn(EventEnvelope, &'a DatabaseTransaction) -> InlineSubscriberFuture<'a>
        + Send
        + Sync,
>;
type DeferredHandler = Arc<
    dyn Fn(
            EventEnvelope,
            DatabaseConnection,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
enum SubscriberCallback {
    Inline(InlineHandler),
    Deferred(DeferredHandler),
}

/// One plugin subscriber and its exact event-name filter.
#[derive(Clone)]
pub struct SubscriberRegistration {
    plugin_name: Option<&'static str>,
    name: &'static str,
    event_names: Vec<&'static str>,
    callback: SubscriberCallback,
}

impl SubscriberRegistration {
    /// Register an observe-only subscriber that shares the action transaction.
    ///
    /// Inline subscribers may only write through the supplied transaction. They
    /// must not perform network or filesystem I/O and cannot emit another event.
    pub fn inline<I, F>(name: &'static str, event_names: I, handler: F) -> Self
    where
        I: IntoIterator<Item = &'static str>,
        F: for<'a> Fn(EventEnvelope, &'a DatabaseTransaction) -> InlineSubscriberFuture<'a>
            + Send
            + Sync
            + 'static,
    {
        Self {
            plugin_name: None,
            name,
            event_names: event_names.into_iter().collect(),
            callback: SubscriberCallback::Inline(Arc::new(handler)),
        }
    }

    /// Register a subscriber delivered from the outbox after commit.
    pub fn deferred<I, F, Fut>(name: &'static str, event_names: I, handler: F) -> Self
    where
        I: IntoIterator<Item = &'static str>,
        F: Fn(EventEnvelope, DatabaseConnection) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        Self {
            plugin_name: None,
            name,
            event_names: event_names.into_iter().collect(),
            callback: SubscriberCallback::Deferred(Arc::new(move |event, db| {
                Box::pin(handler(event, db))
            })),
        }
    }

    /// Assign the owning plugin while application metadata is assembled.
    #[must_use]
    pub fn for_plugin(mut self, plugin_name: &'static str) -> Self {
        self.plugin_name = Some(plugin_name);
        self
    }

    pub fn mode(&self) -> SubscriberMode {
        match self.callback {
            SubscriberCallback::Inline(_) => SubscriberMode::Inline,
            SubscriberCallback::Deferred(_) => SubscriberMode::Deferred,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    fn qualified_name(&self) -> Result<String, EventError> {
        let plugin = self.plugin_name.ok_or_else(|| {
            EventError::InvalidRegistration(format!(
                "subscriber `{}` is not assigned to a plugin",
                self.name
            ))
        })?;
        Ok(format!("{plugin}:{}", self.name))
    }

    fn matches(&self, event_name: &str) -> bool {
        self.event_names.is_empty() || self.event_names.contains(&event_name)
    }
}

/// Failures from event persistence or delivery.
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("event payload could not be serialized: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error("{mode:?} subscriber `{subscriber}` failed: {message}")]
    SubscriberFailed {
        subscriber: String,
        mode: SubscriberMode,
        message: String,
    },
    #[error("inline subscribers cannot emit events")]
    InlineEmission,
    #[error("event transaction cannot commit after a failed emit")]
    FailedTransaction,
    #[error("invalid subscriber registration: {0}")]
    InvalidRegistration(String),
}

struct EventBusInner {
    db: DatabaseConnection,
    subscribers: Vec<SubscriberRegistration>,
    notify: Notify,
    poll_interval: Duration,
}

/// Cloneable Axum extension for transactional event emission.
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

impl EventBus {
    pub fn new(
        db: DatabaseConnection,
        subscribers: impl IntoIterator<Item = SubscriberRegistration>,
    ) -> Result<Self, EventError> {
        Self::with_poll_interval(db, subscribers, DEFAULT_POLL_INTERVAL)
    }

    fn with_poll_interval(
        db: DatabaseConnection,
        subscribers: impl IntoIterator<Item = SubscriberRegistration>,
        poll_interval: Duration,
    ) -> Result<Self, EventError> {
        let subscribers: Vec<_> = subscribers.into_iter().collect();
        let mut names = HashSet::new();
        for subscriber in &subscribers {
            let name = subscriber.qualified_name()?;
            if !names.insert(name.clone()) {
                return Err(EventError::InvalidRegistration(format!(
                    "duplicate subscriber `{name}`"
                )));
            }
        }

        Ok(Self {
            inner: Arc::new(EventBusInner {
                db,
                subscribers,
                notify: Notify::new(),
                poll_interval,
            }),
        })
    }

    /// Begin the only transaction type that can emit an event.
    pub async fn begin(&self) -> Result<EventTransaction, EventError> {
        if INLINE_DISPATCH.try_with(|()| ()).is_ok() {
            return Err(EventError::InlineEmission);
        }
        Ok(EventTransaction {
            bus: self.clone(),
            transaction: Some(self.inner.db.begin().await?),
            failed: false,
        })
    }

    async fn emit<E: Event>(
        &self,
        transaction: &DatabaseTransaction,
        event: &E,
    ) -> Result<EventEnvelope, EventError> {
        if INLINE_DISPATCH.try_with(|()| ()).is_ok() {
            return Err(EventError::InlineEmission);
        }

        let now = chrono::Utc::now();
        let stored = events::ActiveModel {
            name: Set(E::NAME.to_string()),
            payload: Set(serde_json::to_value(event)?),
            created_at: Set(now.into()),
            processed_at: Set(None),
            delivery_attempts: Set(0),
            available_at: Set(now.into()),
            last_error: Set(None),
            ..Default::default()
        }
        .insert(transaction)
        .await?;
        let envelope = EventEnvelope::from(stored);

        for subscriber in self.inner.subscribers.iter().filter(|subscriber| {
            subscriber.mode() == SubscriberMode::Inline && subscriber.matches(E::NAME)
        }) {
            let SubscriberCallback::Inline(handler) = &subscriber.callback else {
                unreachable!("subscriber mode and callback must agree");
            };
            if let Err(error) = INLINE_DISPATCH
                .scope((), handler(envelope.clone(), transaction))
                .await
            {
                return Err(EventError::SubscriberFailed {
                    subscriber: subscriber.qualified_name()?,
                    mode: SubscriberMode::Inline,
                    message: error.to_string(),
                });
            }
        }

        Ok(envelope)
    }

    /// Deliver one bounded batch of committed outbox rows.
    ///
    /// Public for operational drains and deterministic integration tests. The
    /// normal runtime calls this from [`Self::start_drainer`].
    pub async fn drain_once(&self) -> Result<usize, EventError> {
        let now = chrono::Utc::now();
        let pending = events::Entity::find()
            .filter(events::Column::ProcessedAt.is_null())
            .filter(events::Column::AvailableAt.lte(now))
            .order_by_asc(events::Column::Id)
            .limit(BATCH_SIZE)
            .all(&self.inner.db)
            .await?;
        let mut processed = 0;

        for stored in pending {
            let envelope = EventEnvelope::from(stored.clone());
            let mut matched = false;
            let mut failure = None;

            for subscriber in self.inner.subscribers.iter().filter(|subscriber| {
                subscriber.mode() == SubscriberMode::Deferred && subscriber.matches(&envelope.name)
            }) {
                matched = true;
                let SubscriberCallback::Deferred(handler) = &subscriber.callback else {
                    unreachable!("subscriber mode and callback must agree");
                };
                if let Err(error) = handler(envelope.clone(), self.inner.db.clone()).await {
                    let subscriber_name = subscriber.qualified_name()?;
                    tracing::warn!(
                        event_id = envelope.id,
                        event = %envelope.name,
                        subscriber = %subscriber_name,
                        error = %error,
                        "Deferred event subscriber failed"
                    );
                    failure = Some(format!("{subscriber_name}: {error}"));
                    break;
                }
            }

            let mut active: events::ActiveModel = stored.into();
            if matched {
                active.delivery_attempts = Set(envelope_attempts(&active) + 1);
            }

            if let Some(error) = failure {
                active.last_error = Set(Some(truncate_error(error)));
                active.available_at = Set((chrono::Utc::now()
                    + chrono::Duration::from_std(self.inner.poll_interval)
                        .unwrap_or_else(|_| chrono::Duration::seconds(1)))
                .into());
            } else {
                active.processed_at = Set(Some(chrono::Utc::now().into()));
                active.last_error = Set(None);
                processed += 1;
            }
            active.update(&self.inner.db).await?;
        }

        Ok(processed)
    }

    /// Start notify-driven delivery with polling as the correctness fallback.
    pub fn start_drainer(&self) -> JoinHandle<()> {
        let bus = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(bus.inner.poll_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    () = bus.inner.notify.notified() => {}
                    _ = interval.tick() => {}
                }
                if let Err(error) = bus.drain_once().await {
                    tracing::error!(%error, "Event outbox drain failed");
                }
            }
        })
    }

    fn wake(&self) {
        self.inner.notify.notify_one();
    }
}

fn envelope_attempts(active: &events::ActiveModel) -> i32 {
    match &active.delivery_attempts {
        sea_orm::ActiveValue::Set(attempts) | sea_orm::ActiveValue::Unchanged(attempts) => {
            *attempts
        }
        sea_orm::ActiveValue::NotSet => 0,
    }
}

fn truncate_error(error: String) -> String {
    error.chars().take(MAX_ERROR_LENGTH).collect()
}

/// An application transaction that records events and wakes delivery only after commit.
pub struct EventTransaction {
    bus: EventBus,
    transaction: Option<DatabaseTransaction>,
    failed: bool,
}

impl EventTransaction {
    /// Borrow the same transaction for the action's own database work.
    pub fn connection(&self) -> &DatabaseTransaction {
        self.transaction
            .as_ref()
            .expect("event transaction is unavailable after completion")
    }

    /// Serialize, persist, and run inline subscribers in this transaction.
    pub async fn emit<E: Event>(&mut self, event: &E) -> Result<EventEnvelope, EventError> {
        let result = self.bus.emit(self.connection(), event).await;
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    /// Commit the action and only then wake deferred subscribers.
    pub async fn commit(mut self) -> Result<(), EventError> {
        let transaction = self
            .transaction
            .take()
            .expect("event transaction cannot complete twice");
        if self.failed {
            transaction.rollback().await?;
            return Err(EventError::FailedTransaction);
        }

        transaction.commit().await?;
        self.bus.wake();
        Ok(())
    }

    /// Explicitly roll back without waking deferred delivery.
    pub async fn rollback(mut self) -> Result<(), EventError> {
        self.transaction
            .take()
            .expect("event transaction cannot complete twice")
            .rollback()
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        OnceLock,
    };

    use sea_orm::{EntityTrait, PaginatorTrait};

    use super::*;
    use crate::{
        migrate_core,
        models::{events, settings},
    };

    #[derive(Serialize)]
    struct TestEvent {
        value: &'static str,
    }

    impl Event for TestEvent {
        const NAME: &'static str = "test.happened";
    }

    async fn database() -> DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        migrate_core(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn commit_persists_a_typed_event() {
        let db = database().await;
        let bus = EventBus::new(db.clone(), []).unwrap();
        let mut transaction = bus.begin().await.unwrap();

        transaction
            .emit(&TestEvent { value: "typed" })
            .await
            .unwrap();
        transaction.commit().await.unwrap();

        let stored = events::Entity::find().one(&db).await.unwrap().unwrap();
        assert_eq!(stored.name, TestEvent::NAME);
        assert_eq!(stored.payload, serde_json::json!({ "value": "typed" }));
    }

    #[tokio::test]
    async fn inline_failure_forces_the_transaction_to_roll_back() {
        let db = database().await;
        let subscriber = SubscriberRegistration::inline(
            "required-write",
            [TestEvent::NAME],
            |_event, transaction| {
                Box::pin(async move {
                    settings::ActiveModel {
                        plugin_name: Set("inline-proof".to_string()),
                        value: Set(serde_json::json!({ "written": true })),
                        updated_at: Set(chrono::Utc::now().into()),
                    }
                    .insert(transaction)
                    .await?;
                    anyhow::bail!("write failed")
                })
            },
        )
        .for_plugin("test-plugin");
        let bus = EventBus::new(db.clone(), [subscriber]).unwrap();
        let mut transaction = bus.begin().await.unwrap();

        let error = transaction
            .emit(&TestEvent { value: "rollback" })
            .await
            .unwrap_err();
        assert!(matches!(error, EventError::SubscriberFailed { .. }));
        assert!(matches!(
            transaction.commit().await,
            Err(EventError::FailedTransaction)
        ));
        assert_eq!(events::Entity::find().count(&db).await.unwrap(), 0);
        assert_eq!(settings::Entity::find().count(&db).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn deferred_delivery_is_woken_only_after_commit() {
        let db = database().await;
        let delivered = Arc::new(AtomicUsize::new(0));
        let delivered_for_handler = Arc::clone(&delivered);
        let subscriber =
            SubscriberRegistration::deferred("counter", [TestEvent::NAME], move |_event, _db| {
                let delivered = Arc::clone(&delivered_for_handler);
                async move {
                    delivered.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .for_plugin("test-plugin");
        let bus = EventBus::with_poll_interval(db, [subscriber], Duration::from_secs(60)).unwrap();
        let drainer = bus.start_drainer();
        let mut transaction = bus.begin().await.unwrap();
        transaction
            .emit(&TestEvent { value: "deferred" })
            .await
            .unwrap();

        assert_eq!(delivered.load(Ordering::SeqCst), 0);
        transaction.commit().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while delivered.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drainer.abort();
        assert_eq!(delivered.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn polling_recovers_committed_rows_without_a_wake_listener() {
        let db = database().await;
        let delivered = Arc::new(AtomicUsize::new(0));
        let delivered_for_handler = Arc::clone(&delivered);
        let subscriber = SubscriberRegistration::deferred(
            "poll-counter",
            [TestEvent::NAME],
            move |_event, _db| {
                let delivered = Arc::clone(&delivered_for_handler);
                async move {
                    delivered.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .for_plugin("test-plugin");
        let mut transaction = EventBus::new(db.clone(), [])
            .unwrap()
            .begin()
            .await
            .unwrap();
        transaction
            .emit(&TestEvent { value: "poll" })
            .await
            .unwrap();
        transaction.commit().await.unwrap();

        let recovery =
            EventBus::with_poll_interval(db, [subscriber], Duration::from_millis(10)).unwrap();
        let drainer = recovery.start_drainer();
        tokio::time::timeout(Duration::from_secs(1), async {
            while delivered.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drainer.abort();
        assert_eq!(delivered.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_deferred_delivery_stays_in_the_outbox() {
        let db = database().await;
        let subscriber = SubscriberRegistration::deferred(
            "failing-delivery",
            [TestEvent::NAME],
            |_event, _db| async { anyhow::bail!("temporary failure") },
        )
        .for_plugin("test-plugin");
        let bus = EventBus::new(db.clone(), [subscriber]).unwrap();
        let mut transaction = bus.begin().await.unwrap();
        transaction
            .emit(&TestEvent { value: "retry" })
            .await
            .unwrap();
        transaction.commit().await.unwrap();

        assert_eq!(bus.drain_once().await.unwrap(), 0);
        let stored = events::Entity::find().one(&db).await.unwrap().unwrap();
        assert!(stored.processed_at.is_none());
        assert_eq!(stored.delivery_attempts, 1);
        assert!(stored.last_error.unwrap().contains("temporary failure"));
    }

    #[tokio::test]
    async fn inline_subscribers_cannot_start_a_nested_event_transaction() {
        let db = database().await;
        let holder = Arc::new(OnceLock::<EventBus>::new());
        let holder_for_handler = Arc::clone(&holder);
        let refused = Arc::new(AtomicBool::new(false));
        let refused_for_handler = Arc::clone(&refused);
        let subscriber = SubscriberRegistration::inline(
            "no-recursion",
            [TestEvent::NAME],
            move |_event, _transaction| {
                let bus = holder_for_handler.get().unwrap().clone();
                let refused = Arc::clone(&refused_for_handler);
                Box::pin(async move {
                    if matches!(bus.begin().await, Err(EventError::InlineEmission)) {
                        refused.store(true, Ordering::SeqCst);
                        Ok(())
                    } else {
                        anyhow::bail!("nested emission was not refused")
                    }
                })
            },
        )
        .for_plugin("test-plugin");
        let bus = EventBus::new(db, [subscriber]).unwrap();
        holder.set(bus.clone()).ok().unwrap();
        let mut transaction = bus.begin().await.unwrap();

        transaction
            .emit(&TestEvent { value: "outer" })
            .await
            .unwrap();
        transaction.commit().await.unwrap();

        assert!(refused.load(Ordering::SeqCst));
    }

    #[test]
    fn duplicate_subscriber_names_are_rejected_per_plugin() {
        let first = SubscriberRegistration::deferred("same", [], |_event, _db| async { Ok(()) })
            .for_plugin("test-plugin");
        let second = SubscriberRegistration::deferred("same", [], |_event, _db| async { Ok(()) })
            .for_plugin("test-plugin");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let db = runtime.block_on(database());

        assert!(matches!(
            EventBus::new(db, [first, second]),
            Err(EventError::InvalidRegistration(_))
        ));
    }
}
