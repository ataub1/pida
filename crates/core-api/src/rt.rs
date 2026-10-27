//! Laufzeit des Kerns: nativ Tokio (wie bisher), im Browser (`wasm32-unknown-unknown`, Konzept Webdienst) der
//! Ereignis-Takt der Seite – Aufgaben mit `spawn_local`, Zeitgeber aus dem Browser, blockierende Arbeit läuft direkt
//! (die Web-Shell kann den Kern dafür in einem Web Worker betreiben). Nur das, was der Kern braucht.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod imp {
    use std::future::Future;
    use std::time::Duration;

    pub(crate) use tokio::task::JoinError;

    pub(crate) fn spawn<F>(f: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        tokio::spawn(f);
    }

    pub(crate) async fn blocking<T, F>(f: F) -> Result<T, JoinError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(f).await
    }

    pub(crate) fn spawn_blocking<T, F>(f: F)
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(f);
    }

    /// Zeitgeber mit festem Abstand; verpasste Takte werden verschoben, nicht nachgeholt.
    pub(crate) struct Ticker(tokio::time::Interval);

    impl Ticker {
        pub(crate) fn new(period: Duration) -> Ticker {
            let mut i = tokio::time::interval(period);
            i.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            Ticker(i)
        }
        pub(crate) async fn tick(&mut self) {
            self.0.tick().await;
        }
    }

    pub(crate) async fn timeout<F: Future>(d: Duration, f: F) -> Result<F::Output, ()> {
        tokio::time::timeout(d, f).await.map_err(|_| ())
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod imp {
    use std::future::Future;
    use std::time::Duration;

    /// Im Browser bricht eine Panik das Modul ab (kein Abwickeln) – dieser Fehler entsteht nie.
    #[derive(Debug)]
    pub(crate) struct JoinError;

    impl JoinError {
        pub(crate) fn is_panic(&self) -> bool {
            false
        }
        pub(crate) fn into_panic(self) -> Box<dyn std::any::Any + Send> {
            Box::new("abgebrochen")
        }
    }

    pub(crate) fn spawn<F>(f: F)
    where
        F: Future<Output = ()> + 'static,
    {
        wasm_bindgen_futures::spawn_local(f);
    }

    pub(crate) async fn blocking<T, F>(f: F) -> Result<T, JoinError>
    where
        F: FnOnce() -> T,
    {
        // läuft bereits in einer eigenen Aufgabe (`spawn`), also nach dem aktuellen Ereignis
        Ok(f())
    }

    pub(crate) fn spawn_blocking<T, F>(f: F)
    where
        F: FnOnce() -> T + 'static,
        T: 'static,
    {
        spawn(async move {
            let _ = blocking(f).await;
        });
    }

    pub(crate) struct Ticker(u32, bool);

    impl Ticker {
        pub(crate) fn new(period: Duration) -> Ticker {
            Ticker(u32::try_from(period.as_millis()).unwrap_or(u32::MAX), true)
        }
        /// erster Takt sofort (wie `tokio::time::interval`), danach im Abstand
        pub(crate) async fn tick(&mut self) {
            if std::mem::take(&mut self.1) {
                return;
            }
            gloo_timers::future::TimeoutFuture::new(self.0).await;
        }
    }

    pub(crate) async fn timeout<F: Future>(d: Duration, f: F) -> Result<F::Output, ()> {
        let limit = gloo_timers::future::TimeoutFuture::new(u32::try_from(d.as_millis()).unwrap_or(u32::MAX));
        tokio::select! {
            v = f => Ok(v),
            _ = limit => Err(()),
        }
    }
}

pub(crate) use imp::*;
