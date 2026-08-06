> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Script Postman Visualizations

You can use the `pm.visualizer` object and the `pm.getData` method to visually represent your API's request responses with the [Postman Visualizer](/docs/use/send-requests/response-data/visualizer/).

## pm.visualizer

Use the `pm.visualizer.set` method to specify a template you want to use to display response data in the Postman Visualizer:

```js
pm.visualizer.set(layout:String, data:Object, options:Object):Function
```

This method uses the following properties:

* `layout` - (Required) A [Handlebars](https://handlebarsjs.com/) HTML template string.
* `data` - A JSON object that binds to the template. You can access it inside the template string.
* `options` - An [Options object](https://handlebarsjs.com/api-reference/compilation.html) for `Handlebars.compile()`.

Example:

```js
var template = `<p>{{res.info}}</p>`;
pm.visualizer.set(template, {
    res: pm.response.json()
});
```

## pm.getData

Use the `pm.getData` method to get response data inside a Postman Visualizer template string.

```js
pm.getData(callback):Function
```

The `callback` function accepts the following parameters:

* `error` - Any error detail.
* `data` - Data passed to the template by the `pm.visualizer.set` method.

Example:

```js
pm.getData(function (error, data) {
  var value = data.res.info;
});
```