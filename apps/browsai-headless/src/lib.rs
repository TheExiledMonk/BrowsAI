//! Deterministic no-raster host for agent workloads.

use browsai_engine_api::{BrowserEngine, ContextId, ContextOptions, PageId, VirtualViewport};
use browsai_engine_servo::ServoEngine;
use url::Url;

#[derive(Clone, Debug, Default)]
pub struct HeadlessOptions {
    pub viewport: VirtualViewport,
    pub clock_millis: u64,
    pub profile: Option<String>,
    pub profile_identity: Option<browsai_engine_api::ProfileIdentity>,
}

pub struct HeadlessHost {
    pub engine: ServoEngine,
    pub context: ContextId,
    pub options: HeadlessOptions,
}

impl HeadlessHost {
    pub fn start(options: HeadlessOptions) -> Result<Self, browsai_engine_api::EngineError> {
        let mut engine = ServoEngine::new();
        let context = engine.create_context(ContextOptions {
            profile: options.profile.clone(),
            profile_identity: options.profile_identity.clone(),
            headless: true,
            use_real_browser_runtime: false,
            viewport: Some(options.viewport),
            deterministic_clock_millis: Some(options.clock_millis),
            no_raster: true,
        })?;
        Ok(Self {
            engine,
            context,
            options,
        })
    }
    pub fn open(
        &mut self,
        url: Url,
    ) -> Result<(PageId, browsai_engine_api::NavigationHandle), browsai_engine_api::EngineError>
    {
        let page = self.engine.create_page(self.context)?;
        let navigation = self.engine.navigate(page, url)?;
        Ok((page, navigation))
    }

    pub fn advance_clock(&mut self, millis: u64) -> Result<u64, browsai_engine_api::EngineError> {
        let now = self.engine.advance_time(self.context, millis)?;
        self.options.clock_millis = now;
        Ok(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_is_deterministic_and_no_raster() {
        let mut host = HeadlessHost::start(HeadlessOptions::default()).unwrap();
        let (page, navigation) = host
            .open(Url::parse("https://example.test").unwrap())
            .unwrap();
        assert_eq!(page, navigation.page);
        assert!(host.engine.context_options(host.context).unwrap().no_raster);
        assert_eq!(host.advance_clock(25).unwrap(), 25);
        assert_eq!(host.options.clock_millis, 25);
    }
}
