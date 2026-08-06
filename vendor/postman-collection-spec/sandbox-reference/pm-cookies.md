<!-- Source: https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-cookies.md (fetched 2026-08-06). Postman Learning Center "Postman sandbox reference" (v12 docs). -->

> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Access cookies in Postman scripts

You can use the `pm.cookies` methods to access and manipulate cookies for the domain in the request URL. You can also use the `pm.cookies.jar()` methods to access and manipulate cookies for any specified domain.

You can learn about using [cookies](https://www.postmanlabs.com/postman-collection/Cookie.html) and [cookie lists](https://www.postmanlabs.com/postman-collection/CookieList.html) in the Postman Collection SDK.

## pm.cookies methods

Use the `pm.cookies` methods in your scripts to access and manipulate cookies for the requested domain.

### pm.cookies.has(cookieName:String)

Checks if there is a cookie with the specified name for the requested domain.

Returns one of the following:

* `true` - The cookie exists for the requested domain.
* `false` - The cookie doesn't exist for the requested domain.

### pm.cookies.get(cookieName:String)

Gets the value of the specified cookie for the requested domain.

Returns the value of the cookie.

You can append a string to the value of a cookie using the `+` operator before or after the method.

### pm.cookies.toObject()

Gets all cookies and their values for the requested domain.

Returns all cookies and their values as an object.

## pm.cookies.jar methods

Use the `pm.cookies.jar()` methods to specify a domain and access and manipulate its cookies. To enable access to the methods from your scripts, you must first [add a domain to the allowlist](/docs/use/send-requests/response-data/cookies/#access-cookies-in-scripts).

Function calls run asynchronously. Use a callback function to ensure functions run in sequence.

### pm.cookies.jar().set(URL:String, cookieName:String, cookieValue:String, callback(error, cookie))

Sets a cookie with the specified name and value to a domain.

Example:

```js
pm.cookies.jar().set("example.com", "session-id", "abc123", (error, cookie) => {
    if (error) {
      console.error(`An error occurred: ${error}`);
      } else {
        console.log(`Cookie saved: ${cookie}`);
        }
});
```

### pm.cookies.jar().set(URL:String, \{ name:String, value:String, httpOnly:Bool }, callback(error, cookie))

Sets a cookie using a [Cookie](https://www.postmanlabs.com/postman-collection/Cookie.html) object.

Example:

```js
var Cookie = require('postman-collection').Cookie,
    myCookie = new Cookie({
        name: 'session-id',
        value: 'abc123e',
        httpOnly: true
    });

pm.cookies.jar().set("example.com", myCookie, (error, cookie) => {
    if (error) {
      console.error(`An error occurred: ${error}`);
      } else {
        console.log(`Cookie saved: ${cookie}`);
        }
});
```

### pm.cookies.jar().get(URL:String, cookieName:String, callback(error, value))

Gets the value of a cookie at the specified domain, which is available in the callback function.

Returns the value of the cookie.

### pm.cookies.jar().getAll(URL:String, callback(error, cookies))

Gets all cookies for a specified domains, which are available in the callback function.

Returns the name and value of all cookies.

### pm.cookies.jar().unset(URL:String, cookieName:String, callback(error))

Removes a specified cookie from the domain.

### pm.cookies.jar().clear(URL:String, callback(error))

Clears all cookies from the specified domain.

The following example clears all cookies and then sets a cookie for a specified domain, in sequence:

```js
pm.cookies.jar().clear("example.com", (error) => {
    pm.cookies.jar().set("example.com", "session-id", "jkl456p", (error, cookie) => {
        if (error) {
          console.error(`An error occurred: ${error}`);
        } else {
          console.log(`Cookie saved: ${cookie}`);
        }
    })
});
```