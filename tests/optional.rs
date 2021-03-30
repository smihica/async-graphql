use async_graphql::*;

#[tokio::test]
pub async fn test_optional_type() {
    #[derive(InputObject)]
    struct MyInput {
        value: Option<i32>,
    }

    struct Root {
        value1: Option<i32>,
        value2: Option<i32>,
    }

    #[Object]
    impl Root {
        async fn value1(&self) -> Option<i32> {
            self.value1
        }

        async fn value1_ref(&self) -> &Option<i32> {
            &self.value1
        }

        async fn value2(&self) -> Option<i32> {
            self.value2
        }

        async fn value2_ref(&self) -> &Option<i32> {
            &self.value2
        }

        async fn test_arg(&self, input: Option<i32>) -> Option<i32> {
            input
        }

        async fn test_arg2(&self, input: Option<Vec<i32>>) -> Option<Vec<i32>> {
            input
        }

        async fn test_input<'a>(&self, input: MyInput) -> Option<i32> {
            input.value
        }
    }

    let schema = Schema::new(
        Root {
            value1: Some(10),
            value2: None,
        },
        EmptyMutation,
        EmptySubscription,
    );
    let query = r#"{
            value1
            value1Ref
            value2
            value2Ref
            testArg1: testArg(input: 10)
            testArg2: testArg
            testArg3: testArg(input: null)
            testArg21: testArg2(input: null)
            testArg22: testArg2(input: [1, 2, 3])
            testInput1: testInput(input: {value: 10})
            testInput2: testInput(input: {})
            }"#
    .to_owned();
    assert_eq!(
        schema.execute(&query).await.into_result().unwrap().data,
        value!({
            "value1": 10,
            "value1Ref": 10,
            "value2": null,
            "value2Ref": null,
            "testArg1": 10,
            "testArg2": null,
            "testArg3": null,
            "testArg21": null,
            "testArg22": [1, 2, 3],
            "testInput1": 10,
            "testInput2": null,
        })
    );
}
