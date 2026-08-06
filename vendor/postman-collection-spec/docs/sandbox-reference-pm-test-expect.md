> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Writing tests and assertions in scripts

You can add test specifications and assertions to your scripts using the [pm.test](#pmtest) and [pm.expect](#pmexpect) methods.

## pm.test

Use the `pm.test` method to add test specifications inside **Pre-request** or **Post-response** scripts (for HTTP) or **Before invoke**, **On message**, or **After response** scripts (for gRPC).   Tests include a name and assertion function.

A test for HTTP request would look something like the following:

```js
pm.test(testName , specFunction)
```

Where:

* `testName` - A string that contains the test's name.
* `specFunction` - The function defining the test logic.

Postman outputs test results as part of the response. The `pm.test` method returns the `pm` object, enabling you to chain calls.

### Examples

Check whether a response is valid to proceed:

```js
pm.test("response should be okay to process", function () {
  pm.response.to.not.be.error;
  pm.response.to.have.jsonBody('data') // checks existence of a property in the JSON response
  pm.response.to.have.jsonBody('data', { "id" : 1 }); // Performs deep comparison
});
```

Test an asynchronous function using an optional `done` callback:

```js
pm.test('async test', function (done) {
  setTimeout(() => {
    pm.expect(pm.response.code).to.equal(200);
    done();
  }, 1500);
});
```

Get the total number of tests run from a specific location in code:

```js
pm.test.index(); // Number
```

Include multiple assertions to group the related assertions in a single gRPC test:

```js
pm.test("Should receive update events for both users", function () {
  pm.response.messages.to.include({ action: 'update', userId: 'user1' });
  pm.response.messages.to.include({ action: 'update', userId: 'user2' });
});
```

Get the total number of tests run from a specific location in code:

```js
pm.test.index; () =>number
```

Skip a test:

```js
pm.test.skip: (testName, specFunction) => pm
```

## pm.expect

The `pm.expect` method enables you to write assertions on your response data using [ChaiJS expect BDD](https://www.chaijs.com/api/bdd/) syntax.

```js
pm.expect(value: *): Assertion
```

* `value` - What you want to test, where `*` can be any type of value, such as a string or integer.
* `Assertion` - A Chai Assertion object containing chainable methods.

You can also use `pm.response.to.have.*` and `pm.response.to.be.*` to build your assertions.

See [Postman test script examples](/docs/tests-and-scripts/write-scripts/test-examples/) for more assertions.

### Examples

Check if the response returns an HTTP 200 OK response:

```js
pm.test("Response status code is 200", function () {
    // value: pm.response.code (a number)
    // Assertion: checks if it equals 200
    pm.expect(pm.response.code).to.eql(200);
});
```

Check if the request's response body contains the `true` value:

```js
pm.test("Response body has success = true", function () {
    const jsonData = pm.response.json();
    // value: jsonData.success (a boolean)
    // Assertion: checks if it equals true
    pm.expect(jsonData.success).to.eql(true);
});
```