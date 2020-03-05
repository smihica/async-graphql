use crate::model::{__Directive, __Type};
use crate::registry;
use async_graphql_derive::Object;

pub struct __Schema<'a> {
    pub registry: &'a registry::Registry,
    pub query_type: &'a str,
    pub mutation_type: &'a str,
}

#[Object(
    internal,
    desc = "A GraphQL Schema defines the capabilities of a GraphQL server. It exposes all available types and directives on the server, as well as the entry points for query, mutation, and subscription operations."
)]
impl<'a> __Schema<'a> {
    #[field(desc = "A list of all types supported by this server.")]
    async fn types(&self) -> Vec<__Type<'a>> {
        self.registry
            .types
            .values()
            .map(|ty| __Type::new_simple(self.registry, ty))
            .collect()
    }

    #[field(
        name = "queryType",
        desc = "The type that query operations will be rooted at."
    )]
    async fn query_type(&self) -> __Type<'a> {
        __Type::new_simple(self.registry, &self.registry.types[self.query_type])
    }

    #[field(
        name = "mutationType",
        desc = "If this server supports mutation, the type that mutation operations will be rooted at."
    )]
    async fn mutation_type(&self) -> Option<__Type<'a>> {
        Some(__Type::new_simple(
            self.registry,
            &self.registry.types[self.mutation_type],
        ))
    }

    #[field(
        name = "subscriptionType",
        desc = "If this server support subscription, the type that subscription operations will be rooted at."
    )]
    async fn subscription_type(&self) -> Option<__Type<'a>> {
        None
    }

    #[field(desc = "A list of all directives supported by this server.")]
    async fn directives(&self) -> Vec<__Directive<'a>> {
        self.registry
            .directives
            .iter()
            .map(|directive| __Directive {
                registry: &self.registry,
                directive,
            })
            .collect()
    }
}
