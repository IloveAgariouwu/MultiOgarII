#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hotkey {
    Tab,
    StartNewGame,
    ToggleSpectateTarget,
    ChangeUnit,
    Split,
    DoubleSplit,
    TripleSplit,
    QuadSplit,
    SuperQuadSplit,
    QuintupleSplit,
    SextupleSplit,
    SeptupleSplit,
    OctupleSplit,
    Feed,
    FeedOne,
    Parallel,
    InfernoSplit,
    TestSplit,
    OverlapSplit,
    OverlapSplitOnce,
}

impl Hotkey {
    pub(crate) const ALL: [Hotkey; 20] = [
        Hotkey::Tab,
        Hotkey::StartNewGame,
        Hotkey::ToggleSpectateTarget,
        Hotkey::ChangeUnit,
        Hotkey::Split,
        Hotkey::DoubleSplit,
        Hotkey::TripleSplit,
        Hotkey::QuadSplit,
        Hotkey::SuperQuadSplit,
        Hotkey::QuintupleSplit,
        Hotkey::SextupleSplit,
        Hotkey::SeptupleSplit,
        Hotkey::OctupleSplit,
        Hotkey::Feed,
        Hotkey::FeedOne,
        Hotkey::Parallel,
        Hotkey::InfernoSplit,
        Hotkey::TestSplit,
        Hotkey::OverlapSplit,
        Hotkey::OverlapSplitOnce,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Hotkey::Tab => "hkTab",
            Hotkey::StartNewGame => "hkStartNewGame",
            Hotkey::ToggleSpectateTarget => "hkToggelSpectateTarget",
            Hotkey::ChangeUnit => "hkChangeUnit",
            Hotkey::Split => "hkSplit",
            Hotkey::DoubleSplit => "hkDoubleSplit",
            Hotkey::TripleSplit => "hkTripleSplit",
            Hotkey::QuadSplit => "hkQuadSplit",
            Hotkey::SuperQuadSplit => "hkSuperQuadSplit",
            Hotkey::QuintupleSplit => "hkQuintupleSplit",
            Hotkey::SextupleSplit => "hkSextupleSplit",
            Hotkey::SeptupleSplit => "hkSeptupleSplit",
            Hotkey::OctupleSplit => "hkOctupleSplit",
            Hotkey::Feed => "hkFeed",
            Hotkey::FeedOne => "hkFeedOne",
            Hotkey::Parallel => "hkParallel",
            Hotkey::InfernoSplit => "hkInfernoSplit",
            Hotkey::TestSplit => "hkTestSplit",
            Hotkey::OverlapSplit => "hkOverlapSplit",
            Hotkey::OverlapSplitOnce => "hkOverlapSplitOnce",
        }
    }

    pub(crate) fn parse(name: &str) -> Option<Hotkey> {
        Self::ALL.into_iter().find(|h| h.name() == name)
    }

    pub(crate) fn releases_mouse(self) -> bool {
        matches!(
            self,
            Hotkey::Split
                | Hotkey::FeedOne
                | Hotkey::Feed
                | Hotkey::InfernoSplit
                | Hotkey::TestSplit
        )
    }

    pub(crate) fn playing_only(self) -> bool {
        matches!(
            self,
            Hotkey::Split
                | Hotkey::FeedOne
                | Hotkey::QuadSplit
                | Hotkey::QuintupleSplit
                | Hotkey::SextupleSplit
                | Hotkey::SeptupleSplit
                | Hotkey::OctupleSplit
                | Hotkey::InfernoSplit
                | Hotkey::TestSplit
                | Hotkey::OverlapSplit
                | Hotkey::OverlapSplitOnce
        )
    }
}
