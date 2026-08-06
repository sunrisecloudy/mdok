<!-- Source: https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/pm-datasets.md (fetched 2026-08-06). Postman Learning Center "Postman sandbox reference" (v12 docs). -->

> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Manage and use datasets in scripts

The `pm.datasets` function provides access to [datasets](/docs/tests-and-scripts/datasets/overview) from a script or [code mock](/docs/design-apis/mock-apis/local-mock-servers). You can query datasets using SQL to retrieve data when your script or code mock runs. This enables your scripts and code mocks to return dynamic, data-driven responses instead of static values.

All methods are asynchronous and return Promises, so use `await` to access their results. Query results return rows as an async iterable, so use the `for await...of` loop to read the returned rows.

Learn more about [using datasets in scripts and code mocks](/docs/tests-and-scripts/datasets/use-datasets).

The `pm.datasets` function isn't supported in the mock code editor in Cloud View.

## pm.datasets

The `pm.datasets` function provides access to datasets from a script or code mock. You can load a dataset by its ID and then use methods to query the dataset or manage views.

### pm.datasets(datasetId:String)

Loads a dataset and returns a handle you can use to interact with the dataset.

```js
const ds = pm.datasets('menu-id');
```

### dataset.executeView(viewId:String, params?:String\[])

Runs a view that's already defined in the dataset and returns the results. Query rows are returned as an async iterable, so use the `for await...of` loop to read the rows.

```js
const ds = pm.datasets('menu-id');

const result = await ds.executeView(
  'view-id',
  ['pizza']
);

const allRows = [];

for await (const row of result.rows) {
  allRows.push(row);
}
```

### dataset.executeQuery(sql:String, params?:String\[])

Runs a custom SQL query against the dataset and returns the results. Query rows are returned as an async iterable, so use the `for await...of` loop to read the rows.

```js
const ds = pm.datasets('menu-id');

const result = await ds.executeQuery(
  'SELECT * FROM menu WHERE category = ?',
  ['pizza']
);

const allRows = [];

for await (const row of result.rows) {
  allRows.push(row);
}
```