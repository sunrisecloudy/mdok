# MDOK Postman Collection spec coverage

- Spec: https://schema.getpostman.com/json/collection/v2.1.0/collection.json (vendored at `vendor/postman-collection-spec/`)
- Profile: `postman-cli-v1`
- Gate: **PASS** (missing elements: 0)

## Status summary

| Status | Count |
| --- | --- |
| supported | 408 |
| diagnosed | 399 |
| missing | 0 |

## Element table

| Element | Kind | Status | Note |
| --- | --- | --- | --- |
| `collection.info` | container | supported | importer reads this key (static evidence) |
| `collection.info.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.info._postman_id` | leaf | diagnosed | named diagnostic references the element |
| `collection.info.description` | container | supported | importer reads this key (static evidence) |
| `collection.info.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.info.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.info.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.info.version` | container | diagnosed | collection version metadata is diagnosed as MDOK-PM-VERSION |
| `collection.info.version.major` | leaf | diagnosed | collection version metadata is diagnosed as MDOK-PM-VERSION |
| `collection.info.version.minor` | leaf | diagnosed | collection version metadata is diagnosed as MDOK-PM-VERSION |
| `collection.info.version.patch` | leaf | diagnosed | collection version metadata is diagnosed as MDOK-PM-VERSION |
| `collection.info.version.identifier` | leaf | diagnosed | collection version metadata is diagnosed as MDOK-PM-VERSION |
| `collection.info.version.meta` | container | diagnosed | collection version metadata is diagnosed as MDOK-PM-VERSION |
| `collection.info.schema` | leaf | supported | importer reads this key (static evidence) |
| `collection.item` | container | supported | importer reads this key (static evidence) |
| `collection.item.id` | leaf | supported (informational) | internal identifier |
| `collection.item.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.description` | container | supported | lowered into generated Markdown |
| `collection.item.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.variable` | leaf | supported | lowered into generated Markdown |
| `collection.item.variable.id` | leaf | supported (informational) | internal identifier |
| `collection.item.variable.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.variable.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.item.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.item.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.item.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.item.variable.type` | property | supported (informational) | variable type is editor metadata; runtime treats values as strings |
| `collection.item.variable.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.variable.description` | container | supported (informational) | documentation text |
| `collection.item.variable.description.content` | leaf | supported (informational) | documentation text |
| `collection.item.variable.description.type` | leaf | supported (informational) | documentation text |
| `collection.item.variable.description.version` | container | supported (informational) | documentation text |
| `collection.item.variable.system` | leaf | supported (informational) | system-variable flag, editor metadata |
| `collection.item.variable.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.event` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.event.id` | leaf | supported (informational) | internal identifier |
| `collection.item.event.listen` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.event.script` | container | supported | importer reads this key (static evidence) |
| `collection.item.event.script.id` | leaf | supported (informational) | internal identifier |
| `collection.item.event.script.type` | leaf | supported (informational) | script MIME type is always text/javascript |
| `collection.item.event.script.exec` | container | supported | importer reads this key (static evidence) |
| `collection.item.event.script.src` | container | supported (informational) | external script URL is not fetched by the runner in this profile |
| `collection.item.event.script.src.raw` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.protocol` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.host` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.path` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.path.type` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.path.value` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.port` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.query` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.query.key` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.query.value` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.query.disabled` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.query.description` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.query.description.content` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.query.description.type` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.query.description.version` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.hash` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.event.script.src.variable.key` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable.value` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.item.event.script.src.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.item.event.script.src.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.item.event.script.src.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.item.event.script.src.variable.type` | property | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable.name` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable.description` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable.description.content` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable.description.type` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable.description.version` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.src.variable.system` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.event.script.src.variable.disabled` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.event.script.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.event.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.url` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.url.raw` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.protocol` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.host` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.url.path` | container | supported | lowered into generated Markdown |
| `collection.item.request.url.path.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.path.value` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.port` | leaf | supported | lowered into generated Markdown |
| `collection.item.request.url.query` | leaf | supported | lowered into generated Markdown |
| `collection.item.request.url.query.key` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.url.query.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.url.query.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.query.description` | container | supported (informational) | documentation text |
| `collection.item.request.url.query.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.query.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.query.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.url.hash` | leaf | supported | lowered into generated Markdown |
| `collection.item.request.url.variable` | leaf | supported | lowered into generated Markdown |
| `collection.item.request.url.variable.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.request.url.variable.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.variable.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.url.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.url.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.url.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.url.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.url.variable.type` | property | supported | importer reads this key (static evidence) |
| `collection.item.request.url.variable.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.variable.description` | container | supported (informational) | documentation text |
| `collection.item.request.url.variable.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.variable.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.url.variable.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.url.variable.system` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.request.url.variable.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.type=apikey` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.auth.type=awsv4` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.request.auth.type=basic` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.auth.type=bearer` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.auth.type=digest` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.request.auth.type=edgegrid` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.request.auth.type=hawk` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.request.auth.type=noauth` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.auth.type=oauth1` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.request.auth.type=oauth2` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.request.auth.type=ntlm` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.request.auth.type` | property | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.noauth` | container | supported | auth type noauth is lowered as no authentication |
| `collection.item.request.auth.apikey` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.apikey.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.apikey.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.apikey.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.awsv4` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.awsv4.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.awsv4.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.awsv4.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.basic` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.basic.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.basic.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.basic.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.bearer` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.bearer.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.bearer.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.bearer.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.digest` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.digest.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.digest.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.digest.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.edgegrid` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.edgegrid.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.edgegrid.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.edgegrid.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.hawk` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.hawk.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.hawk.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.hawk.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.ntlm` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.ntlm.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.ntlm.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.ntlm.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.oauth1` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.oauth1.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.oauth1.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.oauth1.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.oauth2` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.request.auth.oauth2.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.oauth2.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.auth.oauth2.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.proxy` | container | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.request.proxy.match` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.request.proxy.host` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.request.proxy.port` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.request.proxy.tunnel` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.request.proxy.disabled` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.request.certificate` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.request.certificate.name` | leaf | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.request.certificate.matches` | leaf | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.request.certificate.key` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.request.certificate.key.src` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.request.certificate.cert` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.request.certificate.cert.src` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.request.certificate.passphrase` | leaf | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.request.method` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.description` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.header` | container | supported | lowered into generated Markdown |
| `collection.item.request.header.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.header.value` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.header.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.header.description` | container | supported (informational) | documentation text |
| `collection.item.request.header.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.header.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.header.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.body` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.body.mode=raw` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.body.mode=urlencoded` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.body.mode=formdata` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.body.mode=file` | enum | diagnosed | named diagnostic MDOK-PM-BODY-FILE references the value |
| `collection.item.request.body.mode=graphql` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.body.mode` | property | supported | importer reads this key (static evidence) |
| `collection.item.request.body.raw` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.urlencoded` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.urlencoded.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.urlencoded.value` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.urlencoded.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.urlencoded.description` | container | supported (informational) | documentation text |
| `collection.item.request.body.urlencoded.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.urlencoded.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.urlencoded.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.value` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.type=text` | enum | supported | no diagnostic references this enum value |
| `collection.item.request.body.formdata.type` | property | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.contentType` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.description` | container | supported (informational) | documentation text |
| `collection.item.request.body.formdata.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.src` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.body.formdata.type=file` | enum | diagnosed | named diagnostic MDOK-PM-BODY-FILE references the value |
| `collection.item.request.body.file` | container | diagnosed | file upload bodies are diagnosed as MDOK-PM-BODY-FILE |
| `collection.item.request.body.file.src` | container | diagnosed | file upload bodies are diagnosed as MDOK-PM-BODY-FILE |
| `collection.item.request.body.file.content` | leaf | diagnosed | file upload bodies are diagnosed as MDOK-PM-BODY-FILE |
| `collection.item.request.body.graphql` | container | supported | importer reads this key (static evidence) |
| `collection.item.request.body.options` | container | supported (informational) | body editor metadata (raw language, wrapping); no runtime semantics |
| `collection.item.request.body.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.response` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.id` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.raw` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.protocol` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.host` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.path` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.path.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.path.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.port` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.query` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.query.key` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.query.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.query.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.query.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.query.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.query.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.query.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.hash` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.id` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.url.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.url.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.url.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.url.variable.type` | property | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.name` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.system` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.url.variable.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.type=apikey` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.auth.type=awsv4` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.response.originalRequest.auth.type=basic` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.auth.type=bearer` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.auth.type=digest` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.response.originalRequest.auth.type=edgegrid` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.response.originalRequest.auth.type=hawk` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.response.originalRequest.auth.type=noauth` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.auth.type=oauth1` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.response.originalRequest.auth.type=oauth2` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.response.originalRequest.auth.type=ntlm` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.response.originalRequest.auth.type` | property | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.noauth` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.apikey` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.apikey.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.apikey.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.apikey.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.awsv4` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.awsv4.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.awsv4.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.awsv4.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.basic` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.basic.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.basic.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.basic.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.bearer` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.bearer.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.bearer.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.bearer.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.digest` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.digest.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.digest.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.digest.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.edgegrid` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.edgegrid.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.edgegrid.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.edgegrid.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.hawk` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.hawk.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.hawk.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.hawk.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.ntlm` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.ntlm.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.ntlm.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.ntlm.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.oauth1` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.oauth1.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.oauth1.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.oauth1.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.oauth2` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.oauth2.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.oauth2.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.auth.oauth2.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.proxy` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.proxy.match` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.proxy.host` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.proxy.port` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.proxy.tunnel` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.proxy.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.certificate` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.certificate.name` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.certificate.matches` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.certificate.key` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.certificate.key.src` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.certificate.cert` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.certificate.cert.src` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.certificate.passphrase` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.method` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.header` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.header.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.header.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.header.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.header.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.header.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.header.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.header.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.mode=raw` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.body.mode=urlencoded` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.body.mode=formdata` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.body.mode=file` | enum | diagnosed | named diagnostic MDOK-PM-BODY-FILE references the value |
| `collection.item.response.originalRequest.body.mode=graphql` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.body.mode` | property | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.raw` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.urlencoded` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.urlencoded.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.urlencoded.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.urlencoded.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.urlencoded.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.urlencoded.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.urlencoded.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.urlencoded.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.type=text` | enum | supported | no diagnostic references this enum value |
| `collection.item.response.originalRequest.body.formdata.type` | property | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.contentType` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.src` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.formdata.type=file` | enum | diagnosed | named diagnostic MDOK-PM-BODY-FILE references the value |
| `collection.item.response.originalRequest.body.file` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.file.src` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.file.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.graphql` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.originalRequest.body.options` | container | supported (informational) | body editor metadata (raw language, wrapping); no runtime semantics |
| `collection.item.response.originalRequest.body.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.responseTime` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.timings` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.header` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.header.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.header.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.header.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.header.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.header.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.header.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.header.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.domain` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.expires` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.maxAge` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.hostOnly` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.httpOnly` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.name` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.path` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.secure` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.session` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.cookie.extensions` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.body` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.status` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.response.code` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.protocolProfileBehavior` | container | supported | importer reads this key (static evidence) |
| `collection.item.item` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.item.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.description` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.variable` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.variable.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.item.variable.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.variable.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.variable.type` | property | supported | importer reads this key (static evidence) |
| `collection.item.item.variable.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.variable.description` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.variable.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.variable.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.variable.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.variable.system` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.item.variable.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.event` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.event.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.item.event.listen` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.event.script` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.event.script.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.item.event.script.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.event.script.exec` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.event.script.src` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.raw` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.protocol` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.host` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.path` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.path.type` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.path.value` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.port` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.query` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.query.key` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.query.value` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.query.disabled` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.query.description` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.query.description.content` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.query.description.type` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.query.description.version` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.hash` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.item.event.script.src.variable.key` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable.value` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.event.script.src.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.event.script.src.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.event.script.src.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.event.script.src.variable.type` | property | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable.name` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable.description` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable.description.content` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable.description.type` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable.description.version` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.src.variable.system` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.item.event.script.src.variable.disabled` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.item.item.event.script.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.event.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.raw` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.protocol` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.host` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.path` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.path.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.path.value` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.port` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.query` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.query.key` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.query.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.query.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.query.description` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.query.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.query.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.query.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.hash` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.item.request.url.variable.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.url.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.url.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.url.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.url.variable.type` | property | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable.description` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.url.variable.system` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.item.item.request.url.variable.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.type=apikey` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.auth.type=awsv4` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.request.auth.type=basic` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.auth.type=bearer` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.auth.type=digest` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.request.auth.type=edgegrid` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.request.auth.type=hawk` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.request.auth.type=noauth` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.auth.type=oauth1` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.request.auth.type=oauth2` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.request.auth.type=ntlm` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.request.auth.type` | property | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.noauth` | container | supported | auth type noauth is lowered as no authentication |
| `collection.item.item.request.auth.apikey` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.apikey.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.apikey.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.apikey.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.awsv4` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.awsv4.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.awsv4.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.awsv4.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.basic` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.basic.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.basic.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.basic.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.bearer` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.bearer.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.bearer.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.bearer.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.digest` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.digest.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.digest.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.digest.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.edgegrid` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.edgegrid.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.edgegrid.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.edgegrid.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.hawk` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.hawk.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.hawk.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.hawk.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.ntlm` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.ntlm.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.ntlm.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.ntlm.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.oauth1` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.oauth1.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.oauth1.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.oauth1.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.oauth2` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.item.request.auth.oauth2.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.oauth2.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.auth.oauth2.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.proxy` | container | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.item.request.proxy.match` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.item.request.proxy.host` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.item.request.proxy.port` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.item.request.proxy.tunnel` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.item.request.proxy.disabled` | leaf | diagnosed | request proxy is diagnosed as MDOK-PM-PROXY |
| `collection.item.item.request.certificate` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.item.request.certificate.name` | leaf | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.item.request.certificate.matches` | leaf | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.item.request.certificate.key` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.item.request.certificate.key.src` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.item.request.certificate.cert` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.item.request.certificate.cert.src` | container | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.item.request.certificate.passphrase` | leaf | diagnosed | client certificates are diagnosed as MDOK-PM-CERT |
| `collection.item.item.request.method` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.description` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.header` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.header.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.header.value` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.header.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.header.description` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.header.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.header.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.header.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.mode=raw` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.body.mode=urlencoded` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.body.mode=formdata` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.body.mode=file` | enum | diagnosed | named diagnostic MDOK-PM-BODY-FILE references the value |
| `collection.item.item.request.body.mode=graphql` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.body.mode` | property | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.raw` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.urlencoded` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.urlencoded.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.urlencoded.value` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.urlencoded.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.urlencoded.description` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.urlencoded.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.urlencoded.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.urlencoded.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.value` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.type=text` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.request.body.formdata.type` | property | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.contentType` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.description` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.src` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.formdata.type=file` | enum | diagnosed | named diagnostic MDOK-PM-BODY-FILE references the value |
| `collection.item.item.request.body.file` | container | diagnosed | file upload bodies are diagnosed as MDOK-PM-BODY-FILE |
| `collection.item.item.request.body.file.src` | container | diagnosed | file upload bodies are diagnosed as MDOK-PM-BODY-FILE |
| `collection.item.item.request.body.file.content` | leaf | diagnosed | file upload bodies are diagnosed as MDOK-PM-BODY-FILE |
| `collection.item.item.request.body.graphql` | container | supported | importer reads this key (static evidence) |
| `collection.item.item.request.body.options` | container | supported (informational) | body editor metadata (raw language, wrapping); no runtime semantics |
| `collection.item.item.request.body.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.item.response` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.id` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.raw` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.protocol` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.host` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.path` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.path.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.path.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.port` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.query` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.query.key` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.query.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.query.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.query.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.query.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.query.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.query.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.hash` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.id` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.url.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.url.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.url.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.url.variable.type` | property | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.name` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.system` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.url.variable.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.type=apikey` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.auth.type=awsv4` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.response.originalRequest.auth.type=basic` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.auth.type=bearer` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.auth.type=digest` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.response.originalRequest.auth.type=edgegrid` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.response.originalRequest.auth.type=hawk` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.response.originalRequest.auth.type=noauth` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.auth.type=oauth1` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.response.originalRequest.auth.type=oauth2` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.response.originalRequest.auth.type=ntlm` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.item.response.originalRequest.auth.type` | property | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.noauth` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.apikey` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.apikey.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.apikey.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.apikey.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.awsv4` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.awsv4.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.awsv4.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.awsv4.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.basic` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.basic.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.basic.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.basic.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.bearer` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.bearer.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.bearer.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.bearer.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.digest` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.digest.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.digest.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.digest.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.edgegrid` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.edgegrid.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.edgegrid.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.edgegrid.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.hawk` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.hawk.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.hawk.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.hawk.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.ntlm` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.ntlm.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.ntlm.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.ntlm.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.oauth1` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.oauth1.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.oauth1.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.oauth1.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.oauth2` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.oauth2.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.oauth2.value` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.auth.oauth2.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.proxy` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.proxy.match` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.proxy.host` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.proxy.port` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.proxy.tunnel` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.proxy.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.certificate` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.certificate.name` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.certificate.matches` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.certificate.key` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.certificate.key.src` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.certificate.cert` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.certificate.cert.src` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.certificate.passphrase` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.method` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.header` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.header.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.header.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.header.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.header.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.header.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.header.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.header.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.mode=raw` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.body.mode=urlencoded` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.body.mode=formdata` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.body.mode=file` | enum | diagnosed | named diagnostic MDOK-PM-BODY-FILE references the value |
| `collection.item.item.response.originalRequest.body.mode=graphql` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.body.mode` | property | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.raw` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.urlencoded` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.urlencoded.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.urlencoded.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.urlencoded.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.urlencoded.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.urlencoded.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.urlencoded.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.urlencoded.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.type=text` | enum | supported | no diagnostic references this enum value |
| `collection.item.item.response.originalRequest.body.formdata.type` | property | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.contentType` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.src` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.formdata.type=file` | enum | diagnosed | named diagnostic MDOK-PM-BODY-FILE references the value |
| `collection.item.item.response.originalRequest.body.file` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.file.src` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.file.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.graphql` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.originalRequest.body.options` | container | supported (informational) | body editor metadata (raw language, wrapping); no runtime semantics |
| `collection.item.item.response.originalRequest.body.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.responseTime` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.timings` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.header` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.header.key` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.header.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.header.disabled` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.header.description` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.header.description.content` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.header.description.type` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.header.description.version` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.domain` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.expires` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.maxAge` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.hostOnly` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.httpOnly` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.name` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.path` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.secure` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.session` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.value` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.cookie.extensions` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.body` | container | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.status` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.response.code` | leaf | diagnosed | response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES |
| `collection.item.item.protocolProfileBehavior` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.type=apikey` | enum | supported | no diagnostic references this enum value |
| `collection.item.auth.type=awsv4` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.auth.type=basic` | enum | supported | no diagnostic references this enum value |
| `collection.item.auth.type=bearer` | enum | supported | no diagnostic references this enum value |
| `collection.item.auth.type=digest` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.auth.type=edgegrid` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.auth.type=hawk` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.auth.type=noauth` | enum | supported | no diagnostic references this enum value |
| `collection.item.auth.type=oauth1` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.auth.type=oauth2` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.auth.type=ntlm` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.item.auth.type` | property | supported | importer reads this key (static evidence) |
| `collection.item.auth.noauth` | container | supported | auth type noauth is lowered as no authentication |
| `collection.item.auth.apikey` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.apikey.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.apikey.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.apikey.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.awsv4` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.awsv4.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.awsv4.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.awsv4.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.basic` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.basic.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.basic.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.basic.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.bearer` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.bearer.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.bearer.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.bearer.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.digest` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.digest.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.digest.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.digest.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.edgegrid` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.edgegrid.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.edgegrid.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.edgegrid.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.hawk` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.hawk.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.hawk.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.hawk.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.ntlm` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.ntlm.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.ntlm.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.ntlm.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.oauth1` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.oauth1.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.oauth1.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.oauth1.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.oauth2` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.item.auth.oauth2.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.item.auth.oauth2.value` | container | supported | importer reads this key (static evidence) |
| `collection.item.auth.oauth2.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.event` | container | supported | importer reads this key (static evidence) |
| `collection.event.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.event.listen` | leaf | supported | importer reads this key (static evidence) |
| `collection.event.script` | container | supported | importer reads this key (static evidence) |
| `collection.event.script.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.event.script.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.event.script.exec` | container | supported | importer reads this key (static evidence) |
| `collection.event.script.src` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.raw` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.protocol` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.host` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.path` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.path.type` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.path.value` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.port` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.query` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.query.key` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.query.value` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.query.disabled` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.query.description` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.query.description.content` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.query.description.type` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.query.description.version` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.hash` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.event.script.src.variable.key` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable.value` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.event.script.src.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.event.script.src.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.event.script.src.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.event.script.src.variable.type` | property | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable.name` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable.description` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable.description.content` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable.description.type` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable.description.version` | container | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.src.variable.system` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.event.script.src.variable.disabled` | leaf | supported (informational) | external script src is not fetched in this profile |
| `collection.event.script.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.event.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.variable` | container | supported | lowered into generated Markdown |
| `collection.variable.id` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.variable.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.variable.value` | container | supported | importer reads this key (static evidence) |
| `collection.variable.type=string` | enum | supported | no diagnostic references this enum value |
| `collection.variable.type=boolean` | enum | supported | no diagnostic references this enum value |
| `collection.variable.type=any` | enum | supported | no diagnostic references this enum value |
| `collection.variable.type=number` | enum | supported | no diagnostic references this enum value |
| `collection.variable.type` | property | supported | importer reads this key (static evidence) |
| `collection.variable.name` | leaf | supported | importer reads this key (static evidence) |
| `collection.variable.description` | container | supported | importer reads this key (static evidence) |
| `collection.variable.description.content` | leaf | supported | importer reads this key (static evidence) |
| `collection.variable.description.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.variable.description.version` | container | supported | importer reads this key (static evidence) |
| `collection.variable.system` | leaf | supported (informational) | internal identifier/metadata, no runtime semantics |
| `collection.variable.disabled` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth` | container | supported | importer reads this key (static evidence) |
| `collection.auth.type=apikey` | enum | supported | no diagnostic references this enum value |
| `collection.auth.type=awsv4` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.auth.type=basic` | enum | supported | no diagnostic references this enum value |
| `collection.auth.type=bearer` | enum | supported | no diagnostic references this enum value |
| `collection.auth.type=digest` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.auth.type=edgegrid` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.auth.type=hawk` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.auth.type=noauth` | enum | supported | no diagnostic references this enum value |
| `collection.auth.type=oauth1` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.auth.type=oauth2` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.auth.type=ntlm` | enum | diagnosed | named diagnostic MDOK-PM-AUTH references the value |
| `collection.auth.type` | property | supported | importer reads this key (static evidence) |
| `collection.auth.noauth` | container | supported | auth type noauth is lowered as no authentication |
| `collection.auth.apikey` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.apikey.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.apikey.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.apikey.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.awsv4` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.awsv4.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.awsv4.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.awsv4.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.basic` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.basic.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.basic.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.basic.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.bearer` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.bearer.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.bearer.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.bearer.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.digest` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.digest.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.digest.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.digest.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.edgegrid` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.edgegrid.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.edgegrid.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.edgegrid.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.hawk` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.hawk.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.hawk.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.hawk.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.ntlm` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.ntlm.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.ntlm.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.ntlm.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.oauth1` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.oauth1.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.oauth1.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.oauth1.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.oauth2` | leaf | supported (container) | object/array whose handled subtree is covered by the importer |
| `collection.auth.oauth2.key` | leaf | supported | importer reads this key (static evidence) |
| `collection.auth.oauth2.value` | container | supported | importer reads this key (static evidence) |
| `collection.auth.oauth2.type` | leaf | supported | importer reads this key (static evidence) |
| `collection.protocolProfileBehavior` | container | supported | importer reads this key (static evidence) |

## pm sandbox surface

- supported paths: 184; modules: ['lodash', 'moment', 'ajv', 'uuid', 'querystring', 'crypto-js']
- documented members: implemented or diagnosable-on-use
  - pm.info: implemented
  - pm.test: implemented
  - pm.expect: implemented
  - pm.request: implemented
  - pm.response: implemented
  - pm.cookies: implemented
  - pm.variables: implemented
  - pm.environment: implemented
  - pm.globals: implemented
  - pm.collectionVariables: implemented
  - pm.iterationData: implemented
  - pm.sendRequest: implemented
  - pm.execution: implemented
  - pm.visualizer: implemented
  - pm.vault: implemented
  - pm.payload: implemented
