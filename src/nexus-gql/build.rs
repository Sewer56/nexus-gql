fn main() {
    // Tell cargo to rerun if GraphQL files change
    println!("cargo:rerun-if-changed=src/schema/");

    // The graphql_client_codegen will handle the code generation
    // based on the #[derive(GraphQLQuery)] annotations in our queries module
}
