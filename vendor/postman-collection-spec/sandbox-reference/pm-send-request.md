<!-- Source: https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-send-request.md (fetched 2026-08-06). Postman Learning Center "Postman sandbox reference" (v12 docs). -->

> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Use scripts to send requests in Postman

Use the `pm.sendRequest` method to send a request asynchronously from a **Pre-request** or **Post-response** script in HTTP requests. For gRPC scripts, you can use the method in **Before invoke**, **On message**, and **After response** scripts.&#x20;

This method enables you to run logic in the background if you are carrying out computation or sending multiple requests at the same time without waiting for each to complete. You can avoid blocking issues by adding a callback function so that your code can respond when Postman receives a response. You can then carry out any more processing you need on the response data.

To send a request in your collection using its request ID, [use the `pm.execution.runRequest` method](/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-execution/#pmexecutionrunrequest-method).

## pm.sendRequest

You can pass the `pm.sendRequest` method a URL string, or you can provide a complete request configuration in JSON including headers, method, body, [and more](http://www.postmanlabs.com/postman-collection/Request.html#~definition).

For more information, see the [Request definition](http://www.postmanlabs.com/postman-collection/Request.html#~definition) and [Response structure](http://www.postmanlabs.com/postman-collection/Response.html) reference documentation.

The following examples demonstrate use of the `pm.sendRequest` method:

### Examples

Example with a plain string URL:

```js
try {
    const response = await pm.sendRequest('https://postman-echo.com/get');

    console.log(response.json());
}
catch (error) {
    console.log(error)
}
```

Example with a full-fledged request:

```js
const postRequest = {
  url: 'https://postman-echo.com/post',
  method: 'POST',
  header: {
    'Content-Type': 'application/json',
    'X-Foo': 'bar'
  },
  body: {
    mode: 'raw',
    raw: JSON.stringify({ key: 'this is json' })
  }
};
pm.sendRequest(postRequest, (error, response) => {
  console.log(error ? error : response.json());
});
```

Example containing a test:

```js
pm.sendRequest('https://postman-echo.com/get', (error, response) => {
  if (error) {
    console.log(error);
  }

  pm.test('response should be okay to process', () => {
    pm.expect(error).to.equal(null);
    pm.expect(response).to.have.property('code', 200);
    pm.expect(response).to.have.property('status', 'OK');
  });
});
```