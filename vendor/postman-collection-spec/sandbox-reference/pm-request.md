<!-- Source: https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-request.md (fetched 2026-08-06). Postman Learning Center "Postman sandbox reference" (v12 docs). -->

> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Reference Postman requests in scripts

The `pm.request` object provides access to the data for the request from a script running within it. `pm.request` is available in both **Before invoke** and **After response** scripts. For a **Pre-request** script, this is the request that's about to run. For a **Post-response** script, this is the request that has already run.

You can use the `pm.request` object pre-request scripts to alter various parts of the request configuration before it runs.

## pm.request

Use the `pm.request` methods in your scripts to access and manipulate collection requests.

See the Postman [Collection SDK Request reference](https://www.postmanlabs.com/postman-collection/Request.html) for more information.

### pm.request.headers.add(header:Header):function

Adds a header with the given name and value for the current request.

### pm.request.headers.remove(headerName:String):function

Deletes the request header with the given name.

### pm.request.headers.upsert(\{key: headerName:String, value: headerValue:String}):function

Inserts the given header name and value if the header doesn't exist. If it exists, the existing header updates with the given value.

## Examples

Add a header with the given name and value for the current request:

```js
pm.request.headers.add({
  key: "client-id",
  value: "abcdef"
});
```

## pm.request properties

The `pm.request` object contains the following properties:

* `pm.request.url:Url` - The request's URL.

* `pm.request.headers:HeaderList` - The list of headers for the current request.

* `pm.request.method:String` - The HTTP request's method.&#x20;

* `pm.request.methodPath` - The package, service, and method name in `packageName.serviceName.methodName` format.&#x20;

* `pm.request.body:RequestBody` - The request body's data. This object is immutable and can't be modified from scripts.

* `pm.request.auth` - The request's authentication details.&#x20;

* `pm.request.metadata` - The list of metadata sent with the request. An individual metadata item is an object containing the `key` and `value` properties. For example, `PropertyList<{ key: string, value: string }>`.

* `pm.request.messages` - The list of outgoing messages.  An individual message is a [`PropertyList`](https://www.postmanlabs.com/postman-collection/PropertyList.html) object with the following properties:

  * `data` - The contents of the sent message.
  * `timestamp` - The time the message was sent, represented as a Date object.

  For requests with unary and server streaming methods, `pm.request.messages` contains only one message at index `0`, which can be accessed as `pm.request.messages.idx(0)`.

Request mutation isn't supported in the `pm` object.