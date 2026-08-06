> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Persist state across requests in code mocks

The `pm.state` object provides a persistent store for a [code mock](/docs/design-apis/mock-apis/local-mock-servers). You can read, write, and update data across requests, enabling your mock to behave like a real service instead of returning static responses. State is shared and persists beyond a single run until it's cleared using `pm.state.clear()`. All methods are asynchronous and return Promises, so use `await` to access their results.

## pm.state

Use the `pm.state` object to persist and manage data across requests in your code mock.

### pm.state.get(key:String)

Retrieves the value stored for the key in the current mock session. Resolves to the value if found or `undefined` if the key doesn't exist. Use this to read session data and make decisions based on it.

```js
const user = await pm.state.get("user");

if (user) {
  return { status: 200, body: user };
}

return { status: 404, body: { error: "User not found" } };
```

### pm.state.set(key:String, value:Any)

Stores a JSON-serializable value under the key for the current mock session. Use this to create or replace session state.

```js
await pm.state.set("user", {
  id: "123",
  name: "Avery",
  role: "admin"
});

return { status: 201, body: { message: "User created" } };
```

### pm.state.delete(key:String)

Removes the key and its value from the current mock session. This is useful when simulating deletes or cleanup. Resolves to `true` if the key existed.

```js
await pm.state.delete("user");

return { status: 204 };
```

### pm.state.has(key:String)

Checks whether a key exists in the current mock session. Resolves to `true` if the key is present.

```js
if (!(await pm.state.has("cart"))) {
  await pm.state.set("cart", []);
}

return { status: 200, body: { ready: true } };
```

### pm.state.keys()

Returns an array of all keys stored in the session state.

```js
const keys = await pm.state.keys();

return {
  status: 200,
  body: {
    keys
  }
};
```

### pm.state.size()

Returns the number of keys stored in the session state.

```js
const count = await pm.state.size();

return {
  status: 200,
  body: {
    stateEntries: count
  }
};
```

### pm.state.clear()

Removes all keys from the shared state used by the mock server. Use this to reset data between flows or simulate a full reset.

```js
await pm.state.clear();
```

### pm.state.toObject()

Returns the entire session state as a plain JavaScript object. This is useful for debugging or returning the current mock state in one response.

```js
const snapshot = await pm.state.toObject();

return {
  status: 200,
  body: {
    state: snapshot
  }
};
```

### pm.state.increment(key:String, delta?:Number)

Increments the numeric value stored at the given key by the specified delta (defaults to `1`). If the key doesn't exist, it's created and initialized to the incremented value.

```js
await pm.state.increment("retryCount");

const total = await pm.state.increment("totalRequests", 5);

return {
  status: 200,
  body: {
    retryCount: await pm.state.get("retryCount"),
    totalRequests: total
  }
};
```

### pm.state.push(key:String, items:Array)

Appends one or more items to an array stored at the key. If the array doesn't exist yet, it's created automatically.

```js
await pm.state.push("events", {
  type: "user.created",
  userId: "123"
});

return {
  status: 200,
  body: {
    events: await pm.state.get("events")
  }
};
```

### pm.state.addToSet(key:String, item:Any)

Adds an item to an array only if it's not already present. Resolves to `true` if the item was added.

```js
const added = await pm.state.addToSet("roles", "admin");

return {
  status: 200,
  body: {
    added,
    roles: await pm.state.get("roles")
  }
};
```