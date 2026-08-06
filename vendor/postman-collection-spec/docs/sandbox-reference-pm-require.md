> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Import packages into your scripts

You can use the `pm.require` method to import packages from your team's [Package Library](/docs/tests-and-scripts/write-scripts/packages/package-library/) or [external package registries](/docs/tests-and-scripts/write-scripts/packages/external-package-registries/) inside scripts in HTTP, gRPC, and GraphQL requests.

## pm.require

The `pm.require` method accepts the name of a package. Declare the method as a variable if the package has functions or objects you want to call. If the package only has code or instances of the `pm` object, you don't need to declare the method as a variable.

### Examples

Use the following format to import a package from the Package Library:

```js
const variableName = pm.require('@team-domain/package-name');

variableName.functionName()
```

Use the following format to import an external package from a package registry:

```js
// package imported from npm
const npmVariableName = pm.require('npm:package-name@version-number');

npmVariableName.functionName()

// package imported from jsr
const jsrVariableName = pm.require('jsr:package-name@version-number');

jsrVariableName.functionName()
```

## Use global objects

Postman supports the following JavaScript objects globally in your scripts:

* Standard objects
  * [AggregateError](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/AggregateError)
  * [Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array)
  * [ArrayBuffer](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/ArrayBuffer)
  * [Atomics](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Atomics)
  * [BigInt](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigInt)
  * [BigInt64Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigInt64Array)
  * [BigUint64Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigUint64Array)
  * [Boolean](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Boolean)
  * [DataView](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/DataView)
  * [Date](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date)
  * [Error](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error)
  * [EvalError](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/EvalError)
  * [Float32Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Float32Array)
  * [Float64Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Float64Array)
  * [Function](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Function)
  * [Infinity property](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Infinity)
  * [Int8Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Int8Array)
  * [Int16Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Int16Array)
  * [Int32Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Int32Array)
  * [Intl](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl)
  * [JSON](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/JSON)
  * [Map](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Map)
  * [Math](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Math)
  * [NaN property](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/NaN)
  * [Number](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Number)
  * [Object](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object)
  * [Promise](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise)
  * [Proxy](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Proxy)
  * [RangeError](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/RangeError)
  * [ReferenceError](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/ReferenceError)
  * [Reflect](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Reflect)
  * [RegExp](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/RegExp)
  * [Set](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Set)
  * [SharedArrayBuffer](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer)
  * [String](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String)
  * [Symbol](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Symbol)
  * [SyntaxError](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SyntaxError)
  * [TypeError](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/TypeError)
  * [Uint8Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint8Array)
  * [Uint8ClampedArray](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint8ClampedArray)
  * [Uint16Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint16Array)
  * [Uint32Array](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint32Array)
  * [URIError](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/URIError)
  * [WeakMap](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WeakMap)
  * [WeakSet](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WeakSet)
* Document Object Model (DOM) objects
  * [AbortController](https://developer.mozilla.org/en-US/docs/Web/API/AbortController)
  * [AbortSignal](https://developer.mozilla.org/en-US/docs/Web/API/AbortSignal)
  * [DOMException](https://developer.mozilla.org/en-US/docs/Web/API/DOMException)
  * [Event](https://developer.mozilla.org/en-US/docs/Web/API/Event)
  * [EventTarget](https://developer.mozilla.org/en-US/docs/Web/API/EventTarget)
* Encoding objects
  * [atob method](https://developer.mozilla.org/en-US/docs/Web/API/Window/atob)
  * [btoa method](https://developer.mozilla.org/en-US/docs/Web/API/Window/btoa)
  * [TextEncoder](https://developer.mozilla.org/en-US/docs/Web/API/TextEncoder)
  * [TextEncoderStream](https://developer.mozilla.org/en-US/docs/Web/API/TextEncoderStream)
  * [TextDecoder](https://developer.mozilla.org/en-US/docs/Web/API/TextDecoder)
  * [TextDecoderStream](https://developer.mozilla.org/en-US/docs/Web/API/TextDecoderStream)
* File objects
  * [Blob](https://developer.mozilla.org/en-US/docs/Web/API/Blob)
  * [File](https://developer.mozilla.org/en-US/docs/Web/API/File)
* JavaScript objects
  * [decodeURI](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/decodeURI)
  * [decodeURIComponent](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/decodeURIComponent)
  * [encodeURI](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/encodeURI)
  * [encodeURIComponent](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/encodeURIComponent)
  * [escape](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/escape)
  * [isFinite](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/isFinite)
  * [isNaN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/isNaN)
  * [parseFloat](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/parseFloat)
  * [parseInt](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/parseInt)
  * [undefined](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/undefined)
  * [unescape](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/unescape)
* HTML DOM objects
  * [structuredClone method](https://developer.mozilla.org/en-US/docs/Web/API/Window/structuredClone)
  * [queueMicrotask method](https://developer.mozilla.org/en-US/docs/Web/API/Window/queueMicrotask)
* Streams objects
  * [ByteLengthQueuingStrategy](https://developer.mozilla.org/en-US/docs/Web/API/ByteLengthQueuingStrategy)
  * [CountQueuingStrategy](https://developer.mozilla.org/en-US/docs/Web/API/CountQueuingStrategy)
  * [CompressionStream](https://developer.mozilla.org/en-US/docs/Web/API/CompressionStream)
  * [DecompressionStream](https://developer.mozilla.org/en-US/docs/Web/API/DecompressionStream)
  * [ReadableByteStreamController](https://developer.mozilla.org/en-US/docs/Web/API/ReadableByteStreamController)
  * [ReadableStream](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStream)
  * [ReadableStreamBYOBReader](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStreamBYOBReader)
  * [ReadableStreamBYOBRequest](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStreamBYOBRequest)
  * [ReadableStreamDefaultController](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStreamDefaultController)
  * [ReadableStreamDefaultReader](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStreamDefaultReader)
  * [TransformStream](https://developer.mozilla.org/en-US/docs/Web/API/TransformStream)
  * [TransformStreamDefaultController](https://developer.mozilla.org/en-US/docs/Web/API/TransformStreamDefaultController)
  * [WritableStream](https://developer.mozilla.org/en-US/docs/Web/API/WritableStream)
  * [WritableStreamDefaultController](https://developer.mozilla.org/en-US/docs/Web/API/WritableStreamDefaultController)
  * [WritableStreamDefaultWriter](https://developer.mozilla.org/en-US/docs/Web/API/WritableStreamDefaultWriter)
* URL objects
  * [URL](https://developer.mozilla.org/en-US/docs/Web/API/URL)
  * [URLSearchParams](https://developer.mozilla.org/en-US/docs/Web/API/URLSearchParams)
* Web Crypto objects
  * [Crypto](https://developer.mozilla.org/en-US/docs/Web/API/Crypto)
  * [CryptoKey](https://developer.mozilla.org/en-US/docs/Web/API/CryptoKey)
  * [SubtleCrypto](https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto)
  * [crypto property](https://developer.mozilla.org/en-US/docs/Web/API/Window/crypto)

## Use external libraries

The `require` method enables you to use the sandbox built-in library modules. To use a library, call the `require` method, pass the module name as a parameter, and assign the return object from the method to a variable:

```bash
require(moduleName:String):function
```

The supported libraries available for use in the sandbox include:

* [ajv](https://www.npmjs.com/package/ajv/v/6.12.5)
* [chai](https://www.chaijs.com/)
* [cheerio](https://cheerio.js.org/)
* [csv-parse/lib/sync](https://csv.js.org/parse/)
* [lodash](https://lodash.com/)
* [moment](https://momentjs.com/docs/)
* [postman-collection](http://www.postmanlabs.com/postman-collection/)
* [uuid](https://www.npmjs.com/package/uuid)
* [xml2js](https://www.npmjs.com/package/xml2js)

The following libraries are deprecated and no longer supported:

* [atob](https://www.npmjs.com/package/atob) (use the [atob method](#use-global-objects))
* [btoa](https://www.npmjs.com/package/btoa) (use the [btoa method](#use-global-objects))
* [crypto-js](https://www.npmjs.com/package/crypto-js) (use the [Web Crypto objects](#use-global-objects))
* [tv4](https://github.com/geraintluff/tv4) (use the [ajv](https://www.npmjs.com/package/ajv/v/6.12.5) library)

The following NodeJS modules are also available:

* [path](https://nodejs.org/api/path.html)
* [assert](https://nodejs.org/api/assert.html)
* [buffer](https://nodejs.org/api/buffer.html)
* [util](https://nodejs.org/api/util.html)
* [url](https://nodejs.org/api/url.html)
* [punycode](https://nodejs.org/api/punycode.html)
* [querystring](https://nodejs.org/api/querystring.html)
* [string-decoder](https://nodejs.org/api/string_decoder.html)
* [stream](https://nodejs.org/api/stream.html)
* [timers](https://nodejs.org/api/timers.html)
* [events](https://nodejs.org/api/events.html)

Postman doesn't support the following in the buffer module: [isAscii](https://nodejs.org/api/buffer.html#bufferisasciiinput), [isUtf8](https://nodejs.org/api/buffer.html#bufferisutf8input), [resolveObjectURL](https://nodejs.org/api/buffer.html#bufferresolveobjecturlid), [transcode](https://nodejs.org/api/buffer.html#buffertranscodesource-fromenc-toenc), and [copyBytesFrom](https://nodejs.org/api/buffer.html#static-method-buffercopybytesfromview-offset-length).