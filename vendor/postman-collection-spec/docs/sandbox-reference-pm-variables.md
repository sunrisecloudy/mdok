> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Reference variables in Postman scripts

Access and manipulate [variables](/docs/use/send-requests/variables/variables/) in your scripts at a variety of [variable scopes](#variable-scope-precedence) with the `pm` object. You can use the [pm.globals](#pmglobals), [pm.collectionVariables](#pmcollectionvariables), [pm.environment](#pmenvironment), [pm.iterationData](#pmiterationdata), and [pm.variables](#pmvariables) methods to access variables at individual scopes.

Use the [pm.vault](/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-vault/) method to access vault secrets in your Postman Vault.

You can learn about using variables in the [Postman Collection SDK](https://www.postmanlabs.com/postman-collection/Variable.html).

## Variable scope precedence

Variable scope determines the precedence Postman gives to variables when you reference them in your scripts. The following is the variable scope precedence from broadest to narrowest: global, collection, environment, data, and local. Learn more about [variables scopes in Postman](/docs/use/send-requests/variables/variables/#variable-scopes).

When referencing the `pm.variables` method in your scripts, the variable with the closest scope overrides the others. For example, if you have variables named `score` in the current collection and active environment, `pm.variables.get('score')` returns the current value of the environment variable. You can use `pm.variables.set` to create a local variable with a different value, but the value only persists for the current request or collection run.

The following example shows the scope Postman prioritizes when multiple variables are set with the same name:

```js
// collection var 'score' = 1
// environment var 'score' = 2

// first request run
console.log(pm.variables.get('score')); // outputs 2
console.log(pm.collectionVariables.get('score')); // outputs 1
console.log(pm.environment.get('score')); // outputs 2

// second request run
pm.variables.set('score', 3);// local var
console.log(pm.variables.get('score')); // outputs 3

// third request run
console.log(pm.variables.get('score')); // outputs 2
```

## pm.globals

Use the `pm.globals` methods in your scripts to access and manipulate variables at the global scope. You need Editor permissions to edit global variables.

In performance testing, changes to global variables are retained for the duration of the performance run and scoped to each virtual user (VU). Global variables aren’t saved for future runs, and changes made by one VU don’t affect other VUs.

### pm.globals.has(variableName:String)

Checks if there is a global variable with the specified name.

Returns one of the following:

* `true` - The global variable exists.
* `false` - The global variable doesn't exist.

### pm.globals.get(variableName:String)

Gets the value of a global variable with the specified name.

Returns the value of the global variable.

You can append a string to the value of a global variable using the `+` operator before or after the method.

### pm.globals.set(variableName:String, variableValue:\*)

Sets a global variable with the specified name and value.

### pm.globals.replaceIn(variableName:string)

Gets the resolved value of a [dynamic variable](/docs/tests-and-scripts/write-scripts/variables-list/) inside a script using the syntax `{{$dynamicVariableName}}`.

Returns the value of the dynamic variable.

### pm.globals.toObject()

Gets all global variables.

Returns all global variables and their values as an object.

### pm.globals.unset(variableName:String)

Removes a specified global variable.

### pm.globals.clear():function

Clears all global variables from the workspace.

## pm.collectionVariables

Use the `pm.collectionVariables` methods in your scripts to access and manipulate variables in the collection. You need Editor permissions to edit collection variables.

In performance testing, changes to collection variables are retained for the duration of the performance run and scoped to each virtual user (VU). Global variables aren’t saved for future runs, and changes made by one VU don’t affect other VUs.

### pm.collectionVariables.has(variableName:String)

Checks if there is a variable with the specified name in the open collection.

Returns one of the following:

* `true` - The collection variable exists.
* `false` - The collection variable doesn't exist.

### pm.collectionVariables.get(variableName:String)

Gets the value of a variable with the specified name in the open collection.

Returns the value of the collection variable.

You can append a string to the value of a collection variable using the `+` operator before or after the method.

### pm.collectionVariables.set(variableName:String, variableValue:\*)

Sets a variable with the specified name and value in the open collection.

### pm.collectionVariables.replaceIn(variableName:string)

Gets the resolved value of a [dynamic variable](/docs/tests-and-scripts/write-scripts/variables-list/) inside a script using the syntax `{{$dynamicVariableName}}`

Returns the value of the dynamic variable.

### pm.collectionVariables.toObject()

Gets all variables in the open collection.

Returns all collection variables and their values as an object.

### pm.collectionVariables.unset(variableName:String)

Removes a specified variable from the open collection.

### pm.collectionVariables.clear():function

Clears all variables from the open collection.

## pm.environment

Use the `pm.environment` methods in your scripts to access and manipulate variables in the [active environment](/docs/use/send-requests/variables/managing-environments/#switch-between-environments). You need Editor permissions to edit environment variables.

In performance testing, changes to environment variables are retained for the duration of the performance run and scoped to each virtual user (VU). Global variables aren’t saved for future runs, and changes made by one VU don’t affect other VUs.

### pm.environment.has(variableName:String)

Checks if there is a variable with the specified name in the active environment.

Returns one of the following:

* `true` - The environment variable exists.
* `false` - The environment variable doesn't exist.

### pm.environment.get(variableName:String)

Gets the value of a variable with the specified name in the active environment.

Returns the value of the environment variable.

You can append a string to the value of an environment variable using the `+` operator before or after the method.

### pm.environment.set(variableName:String, variableValue:\*)

Sets a variable with the specified name and value in the active environment.

### pm.environment.replaceIn(variableName:string)

Gets the resolved value of a [dynamic variable](/docs/tests-and-scripts/write-scripts/variables-list/) inside a script using the syntax `{{$dynamicVariableName}}`

Returns the value of the dynamic variable.

### pm.environment.toObject()

Gets all variables in the active environment.

Returns all environment variables and their values as an object.

### pm.environment.unset(variableName:String)

Removes a specified variable from the active environment.

### pm.environment.clear():function

Clears all variables from the active environment.

## pm.iterationData

Use the `pm.iterationData` methods in your scripts to access and manipulate variables from [data files during a collection run](/docs/tests-and-scripts/running-collections/working-with-data-files/).

The `pm.iterationData` method isn't available in performance testing. Because performance testing doesn't use a fixed iteration count, data file variables are stored as local variables accessible with `pm.variables` method.

### pm.iterationData.has(variableName:String)

Checks if there is a variable with the specified name in the iteration data file.

Returns one of the following:

* `true` - The data variable exists.
* `false` - The data variable doesn't exist.

### pm.iterationData.get(variableName:String)

Gets the value of a data variable with the specified name in the iteration data file.

Returns the value of the data variable.

You can append a string to the value of a data variable using the `+` operator before or after the method.

### pm.iterationData.toObject()

Gets all data variables in the iteration data file.

Returns all data variables and their values as an object.

### pm.iterationData.toJSON()

Gets all data variables in the iteration data file.

Returns all data variables and their values as JSON.

### pm.iterationData.unset(key:String)

Removes a specified variable from the iteration data during the collection run.

## pm.variables

Use the `pm.variables` methods in your scripts to access and manipulate variables in the narrowest scope and local variables. To learn more, see [Variable scope precedence](#variable-scope-precedence).

Postman doesn't support using `pm.variables` to access and manipulate vault secrets. Use the [pm.vault](/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-vault/) methods.

In performance testing, `pm.variables` methods are scope-limited to the duration of the request in which they are originally defined and can't be accessed by subsequent requests. Changes made by one VU are private and don't affect other VUs.

### pm.variables.has(variableName:String)

Checks if there is a variable with the specified name in any of the scopes, such as the collection or environment scope.

Returns one of the following:

* `true` - The variable exists in one of the scopes.
* `false` - The global variable doesn't exist in any of the scopes.

### pm.variables.get(variableName:String)

Gets the value of a variable with the specified name in the narrowest scope.

Returns the value of the variable in the narrowest scope. For example, if a variable with the same name exists in the collection and environment scopes, Postman returns the value in the active environment.

You can append a string to the value of a variable using the `+` operator before or after the method.

### pm.variables.set(variableName:String, variableValue:\*)

Sets a local variable with the specified name and value.

### pm.variables.replaceIn(variableName:string)

Gets the resolved value of a [dynamic variable](/docs/tests-and-scripts/write-scripts/variables-list/) inside a script using the syntax `{{$dynamicVariableName}}`

Returns the value of the dynamic variable.

### pm.variables.toObject()

Gets all variables in the active environment.

Based on the [order of precedence](#variable-scope-precedence), returns all variables and their values as an object. The object will contain variables from multiple scopes. For example, if there's a variable in the open collection and globals, the object will include both variables.