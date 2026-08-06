<!-- Source: https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-response.md (fetched 2026-08-06). Postman Learning Center "Postman sandbox reference" (v12 docs). -->

> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Reference Postman responses in scripts

The `pm.response` object provides access to the data returned in the response for the current request. This object is only available in **Post-response** scripts.

For more information, see the Postman Collection SDK [Response](https://www.postmanlabs.com/postman-collection/Response.html) reference.

## pm.response

Use the `pm.response` methods in your scripts to access and manipulate responses.

### pm.response.text():Function

Gets the response text string.

### pm.response.json():Function

Gets the response JSON object, which you can use to get the properties received.

## pm.response properties

The `pm.response` object contains the following properties:

* `pm.response.code:Number` - The response status code.

* `pm.response.status:String` - The status text string.

* `pm.response.headers:HeaderList` - The [list of response headers](https://www.postmanlabs.com/postman-collection/HeaderList.html).

* `pm.response.responseTime:Number` - The time the response took to receive in milliseconds. For requests with streaming methods, `responseTime` denotes the total duration for that request execution.

* `pm.response.responseSize:Number`- The size of the response received.

- `pm.response.metadata` - The list of metadata received with the response. An individual metadata item is a [`PropertyList`](https://www.postmanlabs.com/postman-collection/PropertyList.html) object with the `key` and `value` properties.

- `pm.response.trailers` - The list of trailers received with the response. An individual trailer item is a `PropertyList` object with the `key` and `value` properties.

- `pm.response.messages` - The list of outgoing messages.  An individual message is a `PropertyList` object containing the following properties:

  * `data` - The contents of the received message.
  * `timestamp` - The time the message was received, represented as a `Date` object.

  For requests with unary and client streaming methods, `pm.response.messages` contains only one message at index `0`, which can be accessed as `pm.response.messages.idx(0)`.

## Validate response data with a JSON Schema

The `jsonSchema` method enables you to [write a test assertion](/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-test-expect/#pmtest) that validates the response data with a JSON Schema. Postman uses version 6.12.5 of the [Ajv JSON Schema validator](https://www.npmjs.com/package/ajv/v/6.12.5) to validate response data with a JSON Schema.

To write your test assertion, use the `pm.response` object and [Chai Assertion Library BDD](https://www.chaijs.com/api/bdd/) syntax to access the data returned in the response.

The `jsonSchema` method accepts a JSON Schema as an object in the first argument. It also accepts an object of optional [Ajv options](https://www.npmjs.com/package/ajv/v/6.12.5#options) in the second argument:

```js
pm.response.<bdd-syntax>.jsonSchema(schema, options);
```

You can define a JSON Schema in Postman, such as in the **Scripts** tab of a request. Then you can write a test that validates the properties in your response data against the defined JSON Schema.

### Examples

In this example, the JSON Schema requires the response data to contain an `alpha` property that's a Boolean data type. If the response data is missing this property or it's a different data type, the test fails. This example uses BDD syntax `to.have` to express the assertion:

```js
const schema = {
  "type": "object",
  "properties": {
    "alpha": {
      "type": "boolean"
    }
  },
  "required": ["alpha"]
};

pm.test('Response is valid', function() {
  pm.response.to.have.jsonSchema(schema);
});
```