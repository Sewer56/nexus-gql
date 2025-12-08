use graphql_client::GraphQLQuery;

// Custom scalar types
type DateTime = chrono::DateTime<chrono::Utc>;
type BigInt = crate::types::ByteSizeString; // Deserialize BigInt string as ByteSize

// Generate GraphQL queries from schema.json and query files
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema/schema.json",
    query_path = "src/schema/get_popular_mods.graphql",
    response_derives = "Debug,Clone,PartialEq"
)]
pub struct GetPopularModsForGameAndCategoryByEndorsementsDescending;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema/schema.json",
    query_path = "src/schema/get_mod_files.graphql",
    response_derives = "Debug,Clone,PartialEq"
)]
pub struct GetModFiles;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema/schema.json",
    query_path = "src/schema/get_game.graphql",
    response_derives = "Debug,Clone,PartialEq"
)]
pub struct GetGame;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_popular_mods_query_generation() {
        // Test that the query generation works
        let variables =
            get_popular_mods_for_game_and_category_by_endorsements_descending::Variables {
                game_id: "1704".to_string(),
                category_name: "Models and Textures".to_string(),
                count: Some(10),
                offset: Some(0),
            };

        // Should not panic - this tests that code generation worked
        let _query =
            GetPopularModsForGameAndCategoryByEndorsementsDescending::build_query(variables);
    }

    #[test]
    fn get_mod_files_query_generation() {
        let variables = get_mod_files::Variables {
            mod_id: "659".to_string(),
            game_id: "1704".to_string(),
        };

        let _query = GetModFiles::build_query(variables);
    }

    #[test]
    fn get_game_query_generation() {
        let variables = get_game::Variables {
            domain_name: "skyrimspecialedition".to_string(),
        };

        let _query = GetGame::build_query(variables);
    }
}
