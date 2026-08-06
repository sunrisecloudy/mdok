<!-- Source: https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-message.md (fetched 2026-08-06). Postman Learning Center "Postman sandbox reference" (v12 docs). -->

> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Reference message data in scripts

The `pm.message` object provides access to the data returned in the message that's received from the server. `pm.message` is only available in **On message** scripts.

## pm.message properties

The `pm.message` object contains the following properties:

For an incoming message:

* `pm.message: PropertyList<{ data: any, timestamp: Date }>` - An individual message [`PropertyList`](https://www.postmanlabs.com/postman-collection/PropertyList.html) object with the `key` and `value` properties:
* `data` - The received message content.
* `timestamp` - The time the message was received, represented as a `Date` object.