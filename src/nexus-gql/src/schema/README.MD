# To Generate the Schema

`schema.json` is needed to generate the right types at compile time.
Below are the steps to generate it.

## Download the Tool

First install the tool:

```
cargo install graphql_client_cli
```

## Generate the Schema

Then run the following command to generate the code:

```
graphql-client introspect-schema https://api.nexusmods.com/v2/graphql > schema.json
```

This `schema.json` will be used at compile time to resolve the types.