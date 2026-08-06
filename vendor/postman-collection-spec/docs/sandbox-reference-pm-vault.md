> For clean Markdown content of this page, append .md to this URL. For the complete documentation index, see https://learning.postman.com/llms.txt.

# Reference vault secrets in Postman scripts

Access and manipulate [vault secrets](/docs/use/postman-vault/postman-vault-secrets/) in your Postman Local Vault in your scripts with the `pm.vault` methods. You must [enable support for vault secrets in scripts](#enable-support-for-vault-secrets-in-scripts). When you send a request or manually run a collection that uses the `pm.vault` methods, you'll be prompted to [grant or deny](#grant-scripts-access-to-your-vault-secrets) the collection or workspace access to your vault secrets using scripts.

You can use the [`pm.vault` methods](#pmvault) in pre-request and post-response scripts in your HTTP collections and requests and manual collection runs.

Scheduled collection runs, monitors, the Postman CLI, and Newman don't support the `pm.vault` methods. The methods also aren't supported in [multi-protocol](/docs/use/use-collections/add-requests-to-collections/#about-multi-protocol-collections), such as GraphQL and gRPC.

Note the following when using the `pm.vault` methods:

* Use the [Postman desktop app](/docs/getting-started/installation/install-app/) to access [vault secrets linked to external vaults](/docs/use/postman-vault/postman-vault-integrations/).
* Use the [Postman Desktop Agent](/docs/getting-started/basics/about-postman-agent/#postman-desktop-agent) if using the [Postman web app](/docs/getting-started/installation/install-app/#use-the-postman-web-app) to access vault secrets. Postman recommends you [use the latest version](/docs/getting-started/basics/about-postman-agent/#update-the-postman-desktop-agent) of the Postman Desktop Agent to receive recent changes and improvements.

## Enable support for vault secrets in scripts

You must enable support for vault secrets in scripts. You'll be prompted to [grant or deny](#grant-scripts-access-to-your-vault-secrets) a collection or the workspace access to your vault secrets before you can use the [`pm.vault` methods](#pmvault).

If you try to send requests that use `pm.vault` methods without enabling support, you'll receive an error in the Postman Console. Also, any code that comes after a `pm.vault` method won't run.

To enable support for vault secrets in scripts, do the following:

1. [Open your Postman Local Vault](/docs/use/postman-vault/postman-vault-key/).
2. Click <img alt="Setting icon" src="https://assets.postman.com/postman-docs/aether-icons/descriptive-setting-stroke.svg#icon" width="16px" /> **Settings** in the upper right of your local vault.
3. In the **Settings** tab, turn on the toggle next to **Enable support in scripts**.
4. Click **Enable** to confirm.

## Grant scripts access to your vault secrets

Postman only allows you to run the [`pm.vault` methods](#pmvault) from collections or workspaces you've granted access. When you send a request or manually run a collection that uses the methods, you'll be prompted to [grant or deny access](#grant-or-deny-access) to your vault secrets using scripts. You can [manage the collection or workspace's access](#manage-access-to-your-vault-secrets) to your vault secrets from your Postman Local Vault.

You'll only be prompted to grant or deny access if you've [enabled support for vault secrets in scripts](#enable-support-for-vault-secrets-in-scripts).

A new HTTP request not saved to a collection won't be prompted because it'll automatically be granted access to your vault secrets.

### Grant or deny access

You can grant or deny a collection or workspace access to your vault secrets using scripts. To keep vault secrets in your Postman Local Vault secure, make sure to only run scripts you trust.

If you deny the collection or workspace access to your vault secrets, you'll receive an error in the Postman Console when you send a request at the denied level. Also, any code that comes after a `pm.vault` method won't run when you send a request or manually run a collection.

To grant or deny access to your vault secrets using scripts, do the following:

1. Click **Collections** in the sidebar, then open an HTTP collection or request with a script that accesses or manipulates your vault secrets.

2. [Send the request](/docs/use/send-requests/create-requests/request-basics/#send-a-request) or [manually run the collection](/docs/tests-and-scripts/running-collections/intro-to-collection-runs/).

3. Choose the element you'd like to grant or deny access to:

   * **Only this collection** - To grant or deny access to all requests in the collection.
   * **Entire Workspace** - To grant or deny access to all collections and requests in the workspace.

4. Choose if you'd like to **Grant Access** or **Deny Access**.

Once you've chosen to grant or deny access, Postman won't prompt you again at the chosen level. For example, if you denied access to one collection, Postman will prompt you again from another collection in the workspace. You can [manage the collections and workspace](#manage-access-to-your-vault-secrets) you granted or denied access to.

### Manage access to your vault secrets

To manage a collection or workspace's access to your vault secrets, do the following:

1. [Open your Postman Local Vault](/docs/use/postman-vault/postman-vault-key/).
2. Click <img alt="Setting icon" src="https://assets.postman.com/postman-docs/aether-icons/descriptive-setting-stroke.svg#icon" width="16px" /> **Settings** in the upper right of your local vault.
3. Click the **Manage access** tab.
4. Click one of the following next to a collection or workspace, depending on whether you granted or denied access earlier:

   * **Reset Access** - Reset your decision to grant scripts in the collection or workspace access to your vault secrets. When you send a request in the collection or workspace, you'll be prompted again to grant or deny access.
   * **Grant Access** - Grant all scripts in the collection or workspace access to your vault secrets. This option is available if you denied scripts in the collection or workspace access.

## pm.vault

Use the `pm.vault` methods in your scripts to get, set, and remove [vault secrets](/docs/use/postman-vault/postman-vault-secrets/) in your local vault.

The `pm.vault` methods run asynchronously in your scripts. Each method also returns a Promise object that represents the completion or failure of the method. Add the `await` operator before each `pm.vault` method to wait for the Promise and its resulting value. Note that a method without the `await` operator may run in your script, but it may not behave as expected.

Use the syntax shown in the following examples:

```js
console.log(await pm.vault.get("secretKey"));
await pm.vault.set("secretKey", "newValue");
await pm.vault.unset("secretKey");
```

If you log the value of a vault secret using `console.log`, the value will be masked in the Postman Console by default. To unmask the value of vault secrets in the console, [open your local vault](/docs/use/postman-vault/postman-vault-key/), select <img alt="Setting icon" src="https://assets.postman.com/postman-docs/aether-icons/descriptive-setting-stroke.svg#icon" width="16px" /> **Settings**, then turn off the toggle next to **Mask vault secrets** from the **Settings** tab.

### pm.vault.get(secretKey:String)

Gets the value of the vault secret with the specified name in your local vault.

Returns the value of the vault secret.

You can append a string to the value of a vault secret using the `+` operator before or after the method.

### pm.vault.set(secretKey:String, secretValue:\*)

Sets a vault secret with the specified name and value in your local vault.

Postman doesn't support setting the value of [vault secrets linked with external vaults](/docs/use/postman-vault/postman-vault-integrations/).

### pm.vault.unset(secretKey:String)

Removes a specified vault secret from your local vault.