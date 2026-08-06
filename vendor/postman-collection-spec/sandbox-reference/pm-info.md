<!-- Source: https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-info.md (fetched 2026-08-06). Postman Learning Center "Postman sandbox reference" (v12 docs). -->

> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Reference request metadata in scripts

The `pm.info` object provides meta information related to the request and the script itself.

## pm.info properties

The `pm.info` object contains the following properties:

* `pm.info.eventName:String` - For HTTP requests, this is either `prerequest` or `test`, depending on where the script is executing within the request. For gRPC, it's either `beforeInvoke`, `onIncomingMessage`, or `afterResponse`, depending whether script is running a **Before invoke**, **On message**, or **After response** hook, respectively.&#x20;

* `pm.info.iteration:Number` - The value of the current [iteration](/docs/tests-and-scripts/running-collections/intro-to-collection-runs/).

* `pm.info.iterationCount:Number` - The total number of iterations that are scheduled to run.

* `pm.info.requestName:String` - The saved name of the request running.

* `pm.info.requestId:String` - A unique GUID that identifies the running request.