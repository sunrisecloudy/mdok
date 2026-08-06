<!-- Source: https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-mock.md (fetched 2026-08-06). Postman Learning Center "Postman sandbox reference" (v12 docs). -->

> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Reference requests and examples in code mocks

The `pm.mock` object provides structured, Postman-aware functions for matching incoming requests and sending responses with a code mock. The `pm.mock` API can serve responses from your existing saved Postman examples rather than hard-coding everything.

## pm.mock

Use the `pm.mock` methods to match incoming requests and send responses, including responses from your existing saved Postman examples.

In the following examples, `<request-path>` and `<example-path>` are placeholders for the path to a saved request and [example](/docs/use/send-requests/response-data/examples#save-a-response-as-an-example) in your local Git repo. The example's status code, headers, and body are all sent as the response.

Rather than looking up paths manually, Postman provides a searchable dropdown directly in the mock code editor. When you click the argument, a dropdown list displays where you can search your workspace's requests and examples by name or path.

### pm.mock.matchRequest()

Matches an incoming request against a Postman request by its method and path. Returns true if the incoming request matches the specified criteria.

Example:

```js
if (pm.mock.matchRequest('<request-path>', req)) {
  res.status(200).json([{ id: 1, name: 'Alice' }]);
  return;
}
```

### pm.mock.sendExample()

Sends a saved Postman example as the HTTP response. This is the key integration point between your existing Postman collection data and your code mock.

```js
if (pm.mock.matchRequest('<request-path>', req)) {
  pm.mock.sendExample('<example-path>', res);
  return;
}
```

Below is a complete example using the `pm.mock` API:

```js
// Match GET /users and serve the saved "List Users - 200 OK" example
if (pm.mock.matchRequest('<get-users-request-path>', req)) {
  pm.mock.sendExample('<list-users-200-example-path>', res);
  return;
}

// Match GET /users/:id with a path variable
if (pm.mock.matchRequest('<request-path>', req)) {
  if (req.params.id === '999') {
    res.status(404).json({ error: 'User not found' });
  } else {
    pm.mock.sendExample('<get-user-200-example-path>', res);
  }
  return;
}

// Match POST /users
if (pm.mock.matchRequest('<request-path>', req)) {
  pm.mock.sendExample('<create-user-201-example-path>', res);
  return;
}

res.status(404).json({ error: 'Route not matched' });
```

### Path variable matching

The matching algorithm supports path variables, which are URL segments prefixed with `:` that match any value in that position.

Example:

```js wordWrap
// Matches /products/42, /products/abc, /products/anything
if (pm.mock.matchRequest('<request-path>', req)) {
  console.log('Requested product ID:', req.params.id);
  res.status(200).json({ id: req.params.id, name: 'Example Product' });
  return;
}

// Nested path variables also work
if (pm.mock.matchRequest('<request-path>', req)) {
  res.status(200).json({
    orgId: req.params.orgId,
    userId: req.params.userId
  });
  return;
}
```

&#x20;The matching algorithm matches on HTTP method and URL paths only. It does not match on query parameters or request headers.