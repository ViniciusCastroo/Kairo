use url::Url;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchEngine {
    #[default]
    DuckDuckGo,
    Google,
}

impl SearchEngine {
    pub const ALL: [Self; 2] = [Self::DuckDuckGo, Self::Google];

    pub fn id(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "duckduckgo",
            Self::Google => "google",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|engine| engine.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Google => "Google",
        }
    }

    pub fn endpoint(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "https://duckduckgo.com/",
            Self::Google => "https://www.google.com/search",
        }
    }

    pub fn homepage(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "https://duckduckgo.com/",
            Self::Google => "https://www.google.com/",
        }
    }

    pub fn query(self, text: &str) -> Url {
        let mut url = Url::parse(self.endpoint()).expect("URL de busca embutida válida");
        url.query_pairs_mut().append_pair("q", text);
        url
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn both_engines_encode_queries_without_creating_extra_parameters() {
        for engine in super::SearchEngine::ALL {
            let query = "rust & gtk #linux café";
            let url = engine.query(query);
            assert_eq!(
                url.query_pairs().collect::<Vec<_>>(),
                [("q".into(), query.into())]
            );
            assert!(url.fragment().is_none());
            assert_eq!(super::SearchEngine::from_id(engine.id()), Some(engine));
        }
        assert_eq!(
            super::SearchEngine::Google.query("test").host_str(),
            Some("www.google.com")
        );
        assert!(super::SearchEngine::from_id("unknown").is_none());
    }
}
