/*
 * MDOK pm facade prelude (profile postman-cli-v1).
 *
 * Installs the Postman-compatible `pm` object tree, the chai-compatible
 * `pm.expect`, `console`, `require` (pinned registry), and wraps `pm` in a
 * recording guard Proxy so every leaf property access lands in `used_api`
 * (spec section 4). Unknown members on the pm tree are recorded as
 * MDOK-PM-UNSUPPORTED and throw on use.
 *
 * Host functions (installed by Rust) are named __mdok_*; everything else in
 * here is plain QuickJS. This file must not rely on `eval` or `Function`
 * (the hardened profile stubs them out below).
 */
(function () {
  'use strict';

  var PROFILE = 'postman-cli-v1';
  var DATA_GET = Symbol('mdokDataGet');

  // Capture the per-run eval-module token from the host and immediately remove
  // it from the global scope so user script cannot reach it. Only legitimate
  // require() calls inside this closure pass it to __mdok_eval_module. See
  // security finding F1.
  var EVAL_MODULE_TOKEN = '';
  try {
    EVAL_MODULE_TOKEN = String(globalThis.__mdok_eval_token_once || '');
    delete globalThis.__mdok_eval_token_once;
  } catch (e) { EVAL_MODULE_TOKEN = ''; }

  /* ------------------------------------------------------------------ *
   * helpers
   * ------------------------------------------------------------------ */

  function isContainer(v) {
    return v !== null && typeof v === 'object';
  }

  function describe(v) {
    if (v === null) return 'null';
    if (v === undefined) return 'undefined';
    var t = typeof v;
    if (t === 'string') {
      var s = JSON.stringify(v);
      return s.length > 120 ? s.slice(0, 117) + '...' : s;
    }
    if (t === 'number' || t === 'boolean' || t === 'bigint') return String(v);
    if (t === 'function') return '[Function]';
    if (t === 'symbol') return String(v);
    try {
      var j = JSON.stringify(v);
      if (j === undefined) return '[object]';
      return j.length > 120 ? j.slice(0, 117) + '...' : j;
    } catch (e) {
      return '[object]';
    }
  }

  function deepEqual(a, b) {
    if (a === b) return true;
    if (a === null || b === null) return false;
    if (typeof a !== 'object' || typeof b !== 'object') return false;
    var isArrA = Array.isArray(a);
    var isArrB = Array.isArray(b);
    if (isArrA !== isArrB) return false;
    if (isArrA) {
      if (a.length !== b.length) return false;
      for (var i = 0; i < a.length; i++) {
        if (!deepEqual(a[i], b[i])) return false;
      }
      return true;
    }
    var ka = Object.keys(a);
    var kb = Object.keys(b);
    if (ka.length !== kb.length) return false;
    for (var j = 0; j < ka.length; j++) {
      var k = ka[j];
      if (!Object.prototype.hasOwnProperty.call(b, k)) return false;
      if (!deepEqual(a[k], b[k])) return false;
    }
    return true;
  }

  function lookupValue(obj, path) {
    var parts = String(path).replace(/\[(.+?)\]/g, '.$1').split('.');
    var cur = obj;
    for (var i = 0; i < parts.length; i++) {
      var p = parts[i];
      if (p === '') continue;
      if (cur === null || cur === undefined) return undefined;
      cur = cur[p];
    }
    return cur;
  }

  /* ------------------------------------------------------------------ *
   * recording guard proxy
   * ------------------------------------------------------------------ */

  function guard(path, target) {
    return new Proxy(target, {
      get: function (t, prop, receiver) {
        if (typeof prop === 'symbol') return Reflect.get(t, prop, receiver);
        var full = path + '.' + prop;
        if (Object.prototype.hasOwnProperty.call(t, prop)) {
          __mdok_used(full, false);
          var v;
          try {
            v = t[prop];
          } catch (e) {
            __mdok_used(full, true);
            throw e;
          }
          if (!isContainer(v)) __mdok_used(full, true);
          if (isContainer(v)) return guard(full, v);
          return v;
        }
        if (t[DATA_GET]) {
          var data = t[DATA_GET](prop);
          if (data !== undefined) return data;
        }
        __mdok_unsupported(full);
        throw new Error(
          'MDOK-PM-UNSUPPORTED: ' + full + ' is not part of the ' + PROFILE + ' profile'
        );
      },
      has: function (t, prop) {
        if (typeof prop === 'symbol') return Reflect.has(t, prop);
        if (Object.prototype.hasOwnProperty.call(t, prop)) return true;
        if (t[DATA_GET]) return t[DATA_GET](String(prop)) !== undefined;
        return false;
      },
    });
  }

  /* ------------------------------------------------------------------ *
   * AssertionError + chai-style expect
   * ------------------------------------------------------------------ */

  function AssertionError(message) {
    var e = new Error(message);
    e.name = 'AssertionError';
    return e;
  }

  function makeExpect(value) {
    var flags = {
      deep: false,
      not: false,
      nested: false,
      own: false,
      ordered: false,
      any: false,
      all: false,
    };

    function invert(pass) {
      return flags.not ? !pass : pass;
    }

    function check(pass, msg) {
      if (!invert(pass)) {
        throw AssertionError('expected ' + describe(value) + ' ' + msg);
      }
    }

    function chainify(fn) {
      return function () {
        fn.apply(null, arguments);
        return chain;
      };
    }

    /* --- matchers --- */
    var matchers = {
      equal: function (expected) {
        check(flags.deep ? deepEqual(value, expected) : value === expected, 'to equal ' + describe(expected));
      },
      eql: function (expected) {
        check(deepEqual(value, expected), 'to deeply equal ' + describe(expected));
      },
      include: function (expected) {
        var pass = false;
        if (typeof value === 'string') {
          pass = String(value).indexOf(String(expected)) !== -1;
        } else if (Array.isArray(value)) {
          if (flags.deep) {
            for (var i = 0; i < value.length; i++) {
              if (deepEqual(value[i], expected)) { pass = true; break; }
            }
          } else {
            pass = value.indexOf(expected) !== -1;
          }
        } else if (value !== null && typeof value === 'object') {
          if (flags.deep) {
            pass = Object.prototype.hasOwnProperty.call(value, expected) && deepEqual(value[expected], expected);
          } else {
            pass = Object.prototype.hasOwnProperty.call(value, expected);
          }
        }
        check(pass, 'to include ' + describe(expected));
      },
      includes: null,
      contain: null,
      contains: null,
      ownProperty: null,
      ownPropertyDescriptor: null,
      match: function (re) {
        check(re instanceof RegExp ? re.test(String(value)) : String(re).indexOf(String(value)) !== -1,
          'to match ' + describe(re));
      },
      satisfy: function (fn) {
        check(typeof fn === 'function' && !!fn(value), 'to satisfy the given function');
      },
      property: function (name, expected) {
        var target = flags.nested ? lookupValue(value, name) : (value === null || value === undefined ? undefined : value[name]);
        var present = target !== undefined || (value !== null && value !== undefined &&
          Object.prototype.hasOwnProperty.call(value, name));
        if (arguments.length === 1) {
          check(present, 'to have property ' + describe(name));
        } else {
          var eq = flags.deep ? deepEqual(target, expected) : target === expected;
          check(present && eq, 'to have property ' + describe(name) + ' equal ' + describe(expected));
        }
      },
      lengthOf: function (n) {
        check(value != null && value.length === n, 'to have a length of ' + n + ' but got ' + (value == null ? 'undefined' : value.length));
      },
      keys: function () {
        var expected = Array.prototype.slice.call(arguments);
        if (expected.length === 1 && Array.isArray(expected[0])) expected = expected[0];
        var actual = value === null || value === undefined ? [] :
          (Array.isArray(value) ? value : Object.keys(value));
        var all = flags.any ? false : true;
        for (var i = 0; i < expected.length; i++) {
          var has = actual.indexOf(expected[i]) !== -1;
          if (flags.any && has) { all = true; break; }
          if (!flags.any && !has) { all = false; break; }
        }
        check(all, 'to have keys ' + describe(expected));
      },
      members: function (list) {
        var pass = false;
        if (Array.isArray(value) && Array.isArray(list)) {
          pass = true;
          if (!flags.ordered) {
            var remaining = list.slice();
            for (var i = 0; i < value.length; i++) {
              var found = -1;
              for (var j = 0; j < remaining.length; j++) {
                if (deepEqual(value[i], remaining[j])) { found = j; break; }
              }
              if (found === -1) { pass = false; break; }
              remaining.splice(found, 1);
            }
          } else {
            if (value.length !== list.length) {
              pass = false;
            } else {
              for (var k = 0; k < value.length; k++) {
                if (!deepEqual(value[k], list[k])) { pass = false; break; }
              }
            }
          }
        }
        check(pass, 'to have members ' + describe(list));
      },
      oneOf: function (list) {
        var pass = false;
        if (Array.isArray(list)) {
          for (var i = 0; i < list.length; i++) {
            if (deepEqual(value, list[i])) { pass = true; break; }
          }
        }
        check(pass, 'to be one of ' + describe(list));
      },
      above: function (n) { check(value > n, 'to be above ' + n); },
      greaterThan: function (n) { check(value > n, 'to be greater than ' + n); },
      gt: function (n) { check(value > n, 'to be greater than ' + n); },
      below: function (n) { check(value < n, 'to be below ' + n); },
      lessThan: function (n) { check(value < n, 'to be less than ' + n); },
      lt: function (n) { check(value < n, 'to be less than ' + n); },
      least: function (n) { check(value >= n, 'to be at least ' + n); },
      gte: function (n) { check(value >= n, 'to be at least ' + n); },
      most: function (n) { check(value <= n, 'to be at most ' + n); },
      lte: function (n) { check(value <= n, 'to be at most ' + n); },
      within: function (a, b) { check(value >= a && value <= b, 'to be within ' + a + '..' + b); },
      instanceOf: function (k) { check(value instanceof k, 'to be an instance of ' + (k && k.name)); },
      instanceof: function (k) { check(value instanceof k, 'to be an instance of ' + (k && k.name)); },
      a: function (type) { check(matchesType(value, type), 'to be a ' + type); },
      an: function (type) { check(matchesType(value, type), 'to be an ' + type); },
      throw: function () {
        var pass = false;
        if (typeof value === 'function') {
          try { value(); } catch (e) { pass = true; }
        }
        check(pass, 'to throw');
      },
      throws: function () {
        var pass = false;
        if (typeof value === 'function') {
          try { value(); } catch (e) { pass = true; }
        }
        check(pass, 'to throw');
      },
      status: function (expected) {
        var v = value;
        var code = v && typeof v === 'object' ? v.code : undefined;
        var status = v && typeof v === 'object' ? v.status : undefined;
        var pass;
        if (expected instanceof RegExp) {
          pass = (code !== undefined && expected.test(String(code))) ||
            (status != null && expected.test(String(status)));
        } else {
          pass = code === expected || String(status) === String(expected);
        }
        check(pass, 'to have status ' + describe(expected));
      },
      header: function (name) {
        var v = value;
        var pass = false;
        if (v && typeof v === 'object' && v.headers) {
          var h = v.headers;
          var val = typeof h.get === 'function' ? h.get(name) : undefined;
          pass = val !== undefined && val !== null;
        }
        check(pass, 'to have header ' + describe(name));
      },
      jsonBody: function () {
        var pass = false;
        var v = value;
        if (v && typeof v === 'object' && typeof v.json === 'function') {
          try { v.json(); pass = true; } catch (e) { pass = false; }
        }
        check(pass, 'to have a parseable JSON body');
      },
      jsonSchema: function (schema) {
        var pass = true;
        var v = value;
        if (v && typeof v === 'object' && typeof v.json === 'function') {
          try {
            pass = validateSchema(v.json(), schema);
          } catch (e) {
            pass = false;
          }
        }
        check(pass, 'to match the given JSON schema');
      },
    };

    /* --- property assertions (getters) --- */
    var propertyAssertions = {
      true: function () { check(value === true, 'to be true'); },
      false: function () { check(value === false, 'to be false'); },
      null: function () { check(value === null, 'to be null'); },
      undefined: function () { check(value === undefined, 'to be undefined'); },
      ok: function () { check(!!value, 'to be truthy'); },
      empty: function () {
        var pass = value === null || value === undefined || value === '' ||
          (Array.isArray(value) && value.length === 0) ||
          (typeof value === 'object' && Object.keys(value).length === 0);
        check(pass, 'to be empty');
      },
      nan: function () { check(typeof value === 'number' && isNaN(value), 'to be NaN'); },
      exist: function () { check(value !== null && value !== undefined, 'to exist'); },
    };

    var CHAIN_WORDS = ['to', 'be', 'been', 'is', 'that', 'which', 'and', 'has', 'have',
      'with', 'at', 'of', 'same', 'but', 'does', 'still', 'also', 'itself'];
    var FLAG_WORDS = { not: 'not', deep: 'deep', nested: 'nested', own: 'own',
      ordered: 'ordered', any: 'any', all: 'all' };

    var chainTarget = {};
    var chain = new Proxy(chainTarget, {
      get: function (t, prop, receiver) {
        if (typeof prop === 'symbol') return Reflect.get(t, prop, receiver);
        if (Object.prototype.hasOwnProperty.call(t, prop)) {
          return t[prop]; /* method or getter result */
        }
        if (CHAIN_WORDS.indexOf(prop) !== -1) return chain;
        if (Object.prototype.hasOwnProperty.call(FLAG_WORDS, prop)) {
          flags[FLAG_WORDS[prop]] = true;
          return chain;
        }
        if (prop === 'length') return value == null ? undefined : value.length;
        // Standard Object.prototype members (valueOf, toString, ...) behave
        // normally instead of being treated as unknown chai members.
        if (Reflect.has(Object.prototype, prop)) return Reflect.get(t, prop, receiver);
        __mdok_unsupported('pm.expect.' + prop);
        throw new Error(
          'MDOK-PM-UNSUPPORTED: pm.expect.' + prop + ' is not part of the ' + PROFILE + ' profile'
        );
      },
    });

    matchers.includes = matchers.include;
    matchers.contain = matchers.include;
    matchers.contains = matchers.include;
    matchers.ownProperty = matchers.property;
    matchers.ownPropertyDescriptor = matchers.property;
    matchers.nestedProperty = matchers.property;
    matchers.eq = matchers.equal;
    matchers.equals = matchers.equal;
    matchers.eqls = matchers.eql;
    matchers.haveOwnProperty = function (name) {
      var present = value !== null && value !== undefined &&
        Object.prototype.hasOwnProperty.call(value, name);
      check(present, 'to have own property ' + describe(name));
    };
    for (var m in matchers) {
      Object.defineProperty(chainTarget, m, {
        value: chainify(matchers[m]),
        enumerable: false,
        configurable: true,
        writable: true,
      });
    }
    for (var p in propertyAssertions) {
      (function (name, fn) {
        Object.defineProperty(chainTarget, name, {
          get: function () {
            fn();
            return chain;
          },
          enumerable: false,
          configurable: true,
        });
      })(p, propertyAssertions[p]);
    }
    return chain;
  }

  function matchesType(v, type) {
    switch (String(type)) {
      case 'string': return typeof v === 'string';
      case 'number': return typeof v === 'number' && !isNaN(v);
      case 'boolean': return typeof v === 'boolean';
      case 'function': return typeof v === 'function';
      case 'undefined': return v === undefined;
      case 'null': return v === null;
      case 'array': return Array.isArray(v);
      case 'date': return v instanceof Date;
      case 'regexp': return v instanceof RegExp;
      case 'object': return v !== null && typeof v === 'object' && !Array.isArray(v);
      default: return true;
    }
  }

  /* Best-effort JSON-schema validation subset: required/type/properties. */
  function validateSchema(data, schema) {
    if (schema === null || schema === undefined || typeof schema !== 'object' ||
      Array.isArray(schema)) return true;
    if (Array.isArray(schema.required)) {
      if (data === null || typeof data !== 'object' || Array.isArray(data)) return false;
      for (var i = 0; i < schema.required.length; i++) {
        if (!Object.prototype.hasOwnProperty.call(data, schema.required[i])) return false;
      }
    }
    if (schema.type !== undefined && !matchesType(data, schema.type)) return false;
    if (schema.properties && data !== null && typeof data === 'object') {
      for (var key in schema.properties) {
        if (Object.prototype.hasOwnProperty.call(data, key) &&
          !validateSchema(data[key], schema.properties[key])) return false;
      }
    }
    return true;
  }

  /* ------------------------------------------------------------------ *
   * header list + child response helpers
   * ------------------------------------------------------------------ */

  function makeHeaderList(headers) {
    var entries = [];
    var idx = {};
    (headers || []).forEach(function (h) {
      var key = String(h && h.key != null ? h.key : '');
      var val = String(h && h.value != null ? h.value : '');
      var lk = key.toLowerCase();
      if (Object.prototype.hasOwnProperty.call(idx, lk)) {
        entries[idx[lk]].value = val;
      } else {
        idx[lk] = entries.length;
        entries.push({ key: key, value: val });
      }
    });
    var obj = {
      get: function (name) {
        var e = entries[idx[String(name).toLowerCase()]];
        return e ? e.value : undefined;
      },
      has: function (name) {
        return Object.prototype.hasOwnProperty.call(idx, String(name).toLowerCase());
      },
      toObject: function () {
        var out = {};
        entries.forEach(function (e) { out[e.key] = e.value; });
        return out;
      },
      count: function () {
        return entries.length;
      },
    };
    obj[DATA_GET] = function (prop) {
      var e = entries[idx[String(prop).toLowerCase()]];
      return e ? e.value : undefined;
    };
    return obj;
  }

  function makeChildResponse(jsonStr) {
    var d = JSON.parse(jsonStr);
    return {
      code: d.code != null ? d.code : null,
      status: d.status || '',
      headers: makeHeaderList(d.headers || []),
      text: function () { return String(d.body != null ? d.body : ''); },
      json: function () { return JSON.parse(d.body != null ? d.body : ''); },
      responseTime: d.response_time_ms != null ? d.response_time_ms : 0,
      responseSize: d.response_size_bytes != null ? d.response_size_bytes : 0,
    };
  }

  /* ------------------------------------------------------------------ *
   * pm.request / pm.response data
   * ------------------------------------------------------------------ */

  var requestData = JSON.parse(globalThis.__mdok_request_json || 'null');
  var responseData = JSON.parse(globalThis.__mdok_response_json || 'null');

  function makeRequestBody(body) {
    if (!body) return null;
    return {
      mode: body.mode || 'raw',
      raw: body.raw != null ? String(body.raw) : null,
      toJSON: function () {
        return { mode: body.mode || 'raw', raw: body.raw != null ? String(body.raw) : null };
      },
    };
  }

  function makeResponseToChain(resp) {
    function build(negate) {
      function assert(pass, message) {
        if (negate) {
          if (pass) throw AssertionError('expected response NOT ' + message);
        } else if (!pass) {
          throw AssertionError('expected response ' + message);
        }
      }
      var have = {};
      have.status = function (expected) {
        var code = resp.code;
        var status = resp.status;
        var pass;
        if (expected instanceof RegExp) {
          pass = (code !== undefined && code !== null && expected.test(String(code))) ||
            (status != null && status !== '' && expected.test(String(status)));
        } else {
          pass = code === expected || String(status) === String(expected);
        }
        assert(pass, 'to have status ' + describe(expected));
        return have;
      };
      have.header = function (name) {
        assert(resp.headers.has(String(name)), 'to have header ' + describe(name));
        return have;
      };
      have.body = function (str) {
        var text = resp.text();
        var pass = str instanceof RegExp ? str.test(text) : text === String(str);
        assert(pass, 'body to ' + (str instanceof RegExp ? 'match ' : 'equal ') + describe(str));
        return have;
      };
      have.jsonBody = function () {
        var pass = false;
        try { resp.json(); pass = true; } catch (e) { pass = false; }
        assert(pass, 'to have a parseable JSON body');
        return have;
      };
      have.jsonSchema = function (schema) {
        var pass = false;
        try { pass = validateSchema(resp.json(), schema); } catch (e) { pass = false; }
        assert(pass, 'to match the given JSON schema');
        return have;
      };
      ['ok', 'success', 'redirection', 'clientError', 'serverError', 'error'].forEach(function (word) {
        Object.defineProperty(have, word, {
          get: function () {
            var code = resp.code;
            var pass = false;
            switch (word) {
              case 'ok': pass = code != null && code < 400; break;
              case 'success': pass = code != null && code >= 200 && code < 300; break;
              case 'redirection': pass = code != null && code >= 300 && code < 400; break;
              case 'clientError': pass = code != null && code >= 400 && code < 500; break;
              case 'serverError': pass = code != null && code >= 500 && code < 600; break;
              case 'error': pass = code != null && code >= 400; break;
            }
            assert(pass, 'to have ' + word);
            return have;
          },
          enumerable: false,
          configurable: true,
        });
      });
      var be = {};
      ['info', 'ok', 'success', 'redirection', 'clientError', 'serverError', 'error', 'withBody', 'json'].forEach(function (word) {
        Object.defineProperty(be, word, {
          get: function () {
            var code = resp.code;
            var pass = false;
            switch (word) {
              case 'info': pass = code != null && code >= 100 && code < 200; break;
              case 'ok': pass = code != null && code < 400; break;
              case 'success': pass = code != null && code >= 200 && code < 300; break;
              case 'redirection': pass = code != null && code >= 300 && code < 400; break;
              case 'clientError': pass = code != null && code >= 400 && code < 500; break;
              case 'serverError': pass = code != null && code >= 500 && code < 600; break;
              case 'error': pass = code != null && code >= 400; break;
              case 'withBody': pass = resp.text().length > 0; break;
              case 'json': {
                try { resp.json(); pass = true; } catch (e) { pass = false; }
                break;
              }
            }
            assert(pass, 'to be ' + word);
            return be;
          },
          enumerable: false,
          configurable: true,
        });
      });
      return { have: have, be: be };
    }
    var plain = build(false);
    plain.not = build(true);
    return plain;
  }

  /* ------------------------------------------------------------------ *
   * variable scopes
   * ------------------------------------------------------------------ */

  function makeScope(scopeName) {
    return {
      get: function (name) { return __mdok_scope_get(scopeName, String(name)); },
      set: function (name, value) { __mdok_scope_set(scopeName, String(name), value); },
      has: function (name) { return __mdok_scope_has(scopeName, String(name)); },
      unset: function (name) { __mdok_scope_unset(scopeName, String(name)); },
      replaceIn: function (template) { return __mdok_scope_replace(scopeName, String(template)); },
      toObject: function () { return __mdok_scope_to_object(scopeName); },
    };
  }

  var SCOPE_ORDER = ['global', 'collection', 'environment', 'data', 'local'];

  function variablesGet(name) {
    for (var i = 0; i < SCOPE_ORDER.length; i++) {
      var v = __mdok_scope_get(SCOPE_ORDER[i], name);
      if (v !== null && v !== undefined) return v;
    }
    return undefined;
  }

  function replaceTemplate(template, lookup) {
    return String(template).replace(/\{\{([^}]+)\}\}/g, function (m, key) {
      var v = lookup(String(key).trim());
      if (v === undefined || v === null) return m;
      return String(v);
    });
  }

  /* ------------------------------------------------------------------ *
   * pm object tree
   * ------------------------------------------------------------------ */

  var pmRaw = {};

  pmRaw.test = function (name, fn) {
    if (typeof fn !== 'function') {
      __mdok_test(String(name), false, 'pm.test requires a function as its second argument');
      return;
    }
    try {
      fn();
      __mdok_test(String(name), true, null);
    } catch (e) {
      var msg = e && e.message != null ? String(e.message) : String(e);
      __mdok_test(String(name), false, msg);
    }
  };

  pmRaw.expect = function (value) {
    return makeExpect(value);
  };

  pmRaw.info = {
    eventName: String(globalThis.__mdok_phase || ''),
    iteration: 0,
    iterationCount: 1,
    requestName: requestData && requestData.name != null ? String(requestData.name) : '',
    requestId: requestData && requestData.name != null ? String(requestData.name) : '',
  };

  var requestHeaders = makeHeaderList(requestData && requestData.headers);
  var requestBody = makeRequestBody(requestData && requestData.body);
  pmRaw.request = {
    method: requestData && requestData.method ? String(requestData.method).toUpperCase() : 'GET',
    url: requestData && requestData.url != null ? String(requestData.url) : '',
    headers: requestHeaders,
    body: requestBody,
    auth: null,
    data: requestBody,
  };

  var responseObj = null;
  if (responseData) {
    responseObj = {
      code: responseData.code != null ? responseData.code : null,
      status: responseData.status || '',
      responseTime: responseData.response_time_ms != null ? responseData.response_time_ms : 0,
      responseSize: responseData.response_size_bytes != null ? responseData.response_size_bytes : 0,
      headers: makeHeaderList(responseData.headers),
      text: function () { return String(responseData.body != null ? responseData.body : ''); },
      json: function () { return JSON.parse(responseData.body != null ? responseData.body : ''); },
      toJSON: function () {
        return {
          code: responseData.code != null ? responseData.code : null,
          status: responseData.status || '',
          responseTime: responseData.response_time_ms != null ? responseData.response_time_ms : 0,
          responseSize: responseData.response_size_bytes != null ? responseData.response_size_bytes : 0,
          headers: responseData.headers || [],
          body: responseData.body != null ? responseData.body : '',
        };
      },
      responseCode: responseData.code != null ? responseData.code : null,
      to: null,
    };
    responseObj.to = makeResponseToChain(responseObj);
  }
  pmRaw.response = responseObj;
  // Some collections alias the response as `pm.payload` (non-standard, but
  // present in real-world scripts).
  pmRaw.payload = responseObj;

  pmRaw.variables = {
    get: function (name) { return variablesGet(String(name)); },
    set: function (name, value) { __mdok_scope_set('local', String(name), value); },
    has: function (name) {
      var v = variablesGet(String(name));
      return v !== undefined && v !== null;
    },
    unset: function (name) { __mdok_scope_unset('local', String(name)); },
    replaceIn: function (template) {
      return replaceTemplate(template, function (k) { return variablesGet(k); });
    },
    toObject: function () {
      var out = {};
      SCOPE_ORDER.forEach(function (scope) {
        var obj = __mdok_scope_to_object(scope);
        for (var k in obj) {
          if (Object.prototype.hasOwnProperty.call(obj, k)) out[k] = obj[k];
        }
      });
      return out;
    },
  };

  pmRaw.environment = makeScope('environment');
  pmRaw.globals = makeScope('global');
  pmRaw.collectionVariables = makeScope('collection');
  pmRaw.iterationData = makeScope('data');

  var cookieMap = {};
  if (responseData && responseData.headers) {
    responseData.headers.forEach(function (h) {
      if (String(h.key || '').toLowerCase() === 'set-cookie') {
        var parts = String(h.value || '').split(';');
        var pair = parts[0].split('=');
        var name = String(pair[0]).trim();
        if (name) cookieMap[name] = pair.slice(1).join('=');
      }
    });
  }
  pmRaw.cookies = {
    get: function (name) {
      var key = String(name);
      return Object.prototype.hasOwnProperty.call(cookieMap, key) ? cookieMap[key] : undefined;
    },
    has: function (name) {
      return Object.prototype.hasOwnProperty.call(cookieMap, String(name));
    },
    toObject: function () {
      var out = {};
      for (var k in cookieMap) {
        if (Object.prototype.hasOwnProperty.call(cookieMap, k)) out[k] = cookieMap[k];
      }
      return out;
    },
  };

  function normalizeSendOptions(u) {
    if (typeof u === 'string') {
      return { url: u, method: 'GET', header: [], body: null, auth: null };
    }
    if (!u || typeof u !== 'object') {
      throw new Error('pm.sendRequest requires a URL string or an options object');
    }
    var header = [];
    if (Array.isArray(u.header)) {
      u.header.forEach(function (h) {
        if (h && typeof h === 'object') header.push({ key: String(h.key != null ? h.key : ''), value: String(h.value != null ? h.value : '') });
      });
    } else if (u.header && typeof u.header === 'object') {
      for (var k in u.header) {
        if (Object.prototype.hasOwnProperty.call(u.header, k)) {
          header.push({ key: String(k), value: String(u.header[k]) });
        }
      }
    }
    var body = null;
    if (typeof u.body === 'string') {
      body = { mode: 'raw', raw: u.body };
    } else if (u.body && typeof u.body === 'object') {
      body = { mode: u.body.mode || 'raw', raw: u.body.raw != null ? String(u.body.raw) : null };
    }
    var auth = null;
    if (u.auth && typeof u.auth === 'object') {
      try { auth = JSON.parse(JSON.stringify(u.auth)); } catch (e) { auth = null; }
    }
    return {
      url: String(u.url != null ? u.url : ''),
      method: String(u.method || 'GET').toUpperCase(),
      header: header,
      body: body,
      auth: auth,
    };
  }

  pmRaw.sendRequest = function (urlOrOptions, callback) {
    var opts = normalizeSendOptions(urlOrOptions);
    var promise = new Promise(function (resolve, reject) {
      __mdok_send(
        JSON.stringify(opts),
        function (jsonStr) { resolve(makeChildResponse(jsonStr)); },
        function (errMsg) { reject(new Error(errMsg)); }
      );
    });
    if (typeof callback === 'function') {
      promise.then(
        function (res) { callback(null, res); },
        function (err) { callback(err, null); }
      );
    }
    return promise;
  };

  pmRaw.execution = {
    setNextRequest: function (name) {
      __mdok_control('set_next_request', String(name), true);
    },
    skipRequest: function () {
      __mdok_control('skip_request', null, true);
    },
    runRequest: function (name) {
      __mdok_control('run_request', name == null ? null : String(name), false);
      throw new Error(
        'MDOK-PM-UNSUPPORTED: pm.execution.runRequest is a collection-runner-only API in the ' + PROFILE + ' profile'
      );
    },
  };

  pmRaw.visualizer = {
    set: function (template, data) {
      var t = template == null ? '' : String(template);
      var d;
      try {
        d = data == null ? 'null' : JSON.stringify(data);
      } catch (e) {
        d = String(data);
      }
      if (!__mdok_visualizer(t, d)) {
        throw new Error('MDOK-PM-LIMIT: pm.visualizer.set payload exceeds the ' + PROFILE + ' profile limit');
      }
    },
  };

  pmRaw.vault = {
    get: function (name) {
      return new Promise(function (resolve, reject) {
        __mdok_vault(
          String(name),
          function (v) { resolve(v); },
          function (errMsg) { reject(new Error(errMsg)); }
        );
      });
    },
  };

  /* ------------------------------------------------------------------ *
   * console
   * ------------------------------------------------------------------ */

  function stringifyLogArg(v) {
    if (typeof v === 'string') return v;
    if (v === undefined) return 'undefined';
    if (typeof v === 'function') return '[Function]';
    if (v !== null && typeof v === 'object') {
      try {
        var j = JSON.stringify(v);
        return j === undefined ? '[object]' : j;
      } catch (e) {
        return '[object]';
      }
    }
    return String(v);
  }

  var consoleObj = {};
  ['log', 'info', 'warn', 'error', 'debug'].forEach(function (level) {
    consoleObj[level] = function () {
      var parts = [];
      for (var i = 0; i < arguments.length; i++) parts.push(stringifyLogArg(arguments[i]));
      __mdok_log(level, parts.join(' '));
    };
  });
  globalThis.console = consoleObj;

  /* ------------------------------------------------------------------ *
   * require() — pinned registry (lodash)
   * ------------------------------------------------------------------ */

  var moduleCache = {};

  // Internal module shims: Node builtins that vendored bundles probe at load
  // time (for example crypto-js's `require("crypto")` for entropy). They are
  // resolved only while a module file is being evaluated — never recorded in
  // used_api and never advertised in --list-api — so a script's own
  // `require("crypto")` still fails with MDOK-PM-REQUIRE like it does in the
  // real Postman sandbox.
  var internalModules = {
    crypto: {
      getRandomValues: function (arr) {
        for (var i = 0; i < arr.length; i++) arr[i] = Math.floor(Math.random() * 256);
        return arr;
      },
      randomBytes: function (size) {
        var out = new Uint8Array(size);
        for (var i = 0; i < size; i++) out[i] = Math.floor(Math.random() * 256);
        return out;
      },
    },
  };

  function restoreGlobal(name, saved, had) {
    if (had) globalThis[name] = saved;
    else delete globalThis[name];
  }

  function requireModule(name, record, cacheResult) {
    var key = String(name);
    var doCache = cacheResult !== false;
    if (doCache && Object.prototype.hasOwnProperty.call(moduleCache, key)) {
      return moduleCache[key];
    }
    var src = __mdok_require(key, record !== false);
    if (src === null || src === undefined) {
      throw new Error('MDOK-PM-REQUIRE: module "' + key + '" is not available in the ' + PROFILE + ' profile');
    }
    var hadModule = Object.prototype.hasOwnProperty.call(globalThis, 'module');
    var hadExports = Object.prototype.hasOwnProperty.call(globalThis, 'exports');
    var hadGlobal = Object.prototype.hasOwnProperty.call(globalThis, 'global');
    var hadSelf = Object.prototype.hasOwnProperty.call(globalThis, 'self');
    var hadRequire = Object.prototype.hasOwnProperty.call(globalThis, 'require');
    var savedModule = globalThis.module;
    var savedExports = globalThis.exports;
    var savedGlobal = globalThis.global;
    var savedSelf = globalThis.self;
    var savedRequire = globalThis.require;
    // UMD bundles test the bare `module`/`exports` identifiers (sloppy-mode
    // globals), so the shim must use exactly those names.
    globalThis.module = { exports: {} };
    globalThis.exports = globalThis.module.exports;
    globalThis.global = globalThis;
    globalThis.self = globalThis;
    globalThis.require = function (spec) {
      if (Object.prototype.hasOwnProperty.call(internalModules, spec)) {
        return internalModules[spec];
      }
      return requireModule(spec);
    };
    try {
      __mdok_eval_module(EVAL_MODULE_TOKEN, src);
      var result = globalThis.module.exports;
      if (result === undefined || result === null || Object.keys(result).length === 0) {
        // UMD may have fallen back to a global (e.g. `re._ = be`).
        if (globalThis._ !== undefined) result = globalThis._;
      }
      if (doCache) moduleCache[key] = result;
      return result;
    } finally {
      restoreGlobal('module', savedModule, hadModule);
      restoreGlobal('exports', savedExports, hadExports);
      restoreGlobal('global', savedGlobal, hadGlobal);
      restoreGlobal('self', savedSelf, hadSelf);
      restoreGlobal('require', savedRequire, hadRequire);
    }
  }
  globalThis.require = requireModule;

  /* ------------------------------------------------------------------ *
   * timers — setTimeout/setInterval/clear*, pumped by the Rust shell
   * between promise-job drains, bounded by the script deadline
   * ------------------------------------------------------------------ */

  var timerSeq = 0;
  var timers = {};

  function timerNow() { return Date.now(); }

  globalThis.setTimeout = function (fn, ms) {
    if (typeof fn !== 'function') {
      throw new TypeError('setTimeout: first argument must be a function');
    }
    var id = ++timerSeq;
    var delay = Math.max(0, Number(ms) || 0);
    var args = Array.prototype.slice.call(arguments, 2);
    timers[id] = { kind: 'timeout', fn: fn, at: timerNow() + delay, ms: delay, args: args };
    return id;
  };

  globalThis.setInterval = function (fn, ms) {
    if (typeof fn !== 'function') {
      throw new TypeError('setInterval: first argument must be a function');
    }
    var id = ++timerSeq;
    var delay = Math.max(0, Number(ms) || 0);
    var args = Array.prototype.slice.call(arguments, 2);
    timers[id] = { kind: 'interval', fn: fn, at: timerNow() + delay, ms: delay, args: args };
    return id;
  };

  globalThis.clearTimeout = function (id) { delete timers[id]; };
  globalThis.clearInterval = function (id) { delete timers[id]; };

  // Fire every due timer, then report the delay (ms) until the next one
  // (-1 when the queue is empty). The Rust shell calls this repeatedly
  // between promise-job drains; the script deadline bounds the whole pump.
  globalThis.__mdok_drain_timers = function () {
    var now = timerNow();
    var due = [];
    for (var id in timers) {
      if (Object.prototype.hasOwnProperty.call(timers, id) && timers[id].at <= now) {
        due.push(id);
      }
    }
    for (var i = 0; i < due.length; i++) {
      var id = due[i];
      var t = timers[id];
      if (!t) continue;
      if (t.kind === 'interval') {
        t.at = timerNow() + t.ms;
      } else {
        delete timers[id];
      }
      try {
        t.fn.apply(undefined, t.args);
      } catch (e) {
        __mdok_timer_error(String((e && e.message) || e));
      }
    }
    var next = -1;
    now = timerNow();
    for (var id2 in timers) {
      if (!Object.prototype.hasOwnProperty.call(timers, id2)) continue;
      var d = timers[id2].at - now;
      if (next === -1 || d < next) next = d;
    }
    return next;
  };

  // Legacy Postman sandbox global `_` (lodash) — same family as xml2Json.
  // Internal install-time load is neither recorded as script API usage nor
  // cached under the public key, so a script's own require("lodash") still
  // resolves through the normal (recorded) registry path.
  globalThis._ = requireModule('lodash', false, false);

  /* ------------------------------------------------------------------ *
   * legacy Postman sandbox globals (postman-legacy-interface)
   * ------------------------------------------------------------------ */

  var legacyResponseBody = responseData && responseData.body != null ? String(responseData.body) : '';
  var legacyResponseCode = {
    code: responseData && responseData.code != null ? responseData.code : 0,
    name: responseData && responseData.status ? responseData.status : '',
    detail: responseData && responseData.status ? responseData.status : '',
  };
  var legacyResponseHeaders = {};
  if (responseData && responseData.headers) {
    responseData.headers.forEach(function (h) {
      var key = String(h.key || '');
      if (key && !Object.prototype.hasOwnProperty.call(legacyResponseHeaders, key)) {
        legacyResponseHeaders[key] = String(h.value != null ? h.value : '');
      }
    });
  }

  globalThis.responseBody = legacyResponseBody;
  globalThis.responseCode = legacyResponseCode;
  globalThis.responseHeaders = legacyResponseHeaders;
  globalThis.responseTime = responseData && responseData.response_time_ms != null ? responseData.response_time_ms : 0;
  globalThis.iteration = 0;

  // Legacy `tests` object: tests["name"] = boolean records a test result.
  var legacyTestsStore = {};
  globalThis.tests = new Proxy(legacyTestsStore, {
    set: function (t, prop, value) {
      if (typeof prop === 'string') {
        var passed = !!value;
        var error = value === false || value === null || value === undefined ? 'assertion failed' : null;
        __mdok_test(prop, passed, error);
      }
      t[prop] = value;
      return true;
    },
    get: function (t, prop) {
      if (typeof prop === 'symbol') return Reflect.get(t, prop);
      return t[prop];
    },
    has: function (t, prop) {
      return typeof prop === 'string' && Object.prototype.hasOwnProperty.call(t, prop);
    },
  });

  function makeLegacyScope(scopeName) {
    var obj = {
      get: function (k) { return __mdok_scope_get(scopeName, String(k)); },
      set: function (k, v) { __mdok_scope_set(scopeName, String(k), v); },
      unset: function (k) { __mdok_scope_unset(scopeName, String(k)); },
      has: function (k) { return __mdok_scope_has(scopeName, String(k)); },
      replaceIn: function (tpl) { return __mdok_scope_replace(scopeName, String(tpl)); },
      toObject: function () { return __mdok_scope_to_object(scopeName); },
      clear: function () {
        var all = __mdok_scope_to_object(scopeName);
        for (var k in all) {
          if (Object.prototype.hasOwnProperty.call(all, k)) __mdok_scope_unset(scopeName, k);
        }
      },
    };
    return obj;
  }
  globalThis.environment = makeLegacyScope('environment');
  globalThis.globals = makeLegacyScope('global');

  // Legacy `data` (iteration data): `data.get(k)` plus direct property access.
  var legacyData = {
    get: function (k) { return __mdok_scope_get('data', String(k)); },
    toObject: function () { return __mdok_scope_to_object('data'); },
  };
  globalThis.data = new Proxy(legacyData, {
    get: function (t, prop) {
      if (typeof prop === 'symbol') return Reflect.get(t, prop);
      if (Object.prototype.hasOwnProperty.call(t, prop)) return t[prop];
      return __mdok_scope_get('data', String(prop));
    },
    has: function (t, prop) {
      return typeof prop === 'string' &&
        (Object.prototype.hasOwnProperty.call(t, prop) || __mdok_scope_has('data', prop));
    },
  });

  // Legacy `postman` object.
  globalThis.postman = {
    getResponseHeader: function (name) {
      return responseObj ? responseObj.headers.get(String(name)) : undefined;
    },
    setEnvironmentVariable: function (k, v) { __mdok_scope_set('environment', String(k), v); },
    getEnvironmentVariable: function (k) { return __mdok_scope_get('environment', String(k)); },
    clearEnvironmentVariable: function (k) { __mdok_scope_unset('environment', String(k)); },
    clearEnvironmentVariables: function () {
      var all = __mdok_scope_to_object('environment');
      for (var k in all) {
        if (Object.prototype.hasOwnProperty.call(all, k)) __mdok_scope_unset('environment', k);
      }
    },
    setGlobalVariable: function (k, v) { __mdok_scope_set('global', String(k), v); },
    getGlobalVariable: function (k) { return __mdok_scope_get('global', String(k)); },
    clearGlobalVariable: function (k) { __mdok_scope_unset('global', String(k)); },
    clearGlobalVariables: function () {
      var all = __mdok_scope_to_object('global');
      for (var k in all) {
        if (Object.prototype.hasOwnProperty.call(all, k)) __mdok_scope_unset('global', k);
      }
    },
    setNextRequest: function (name) { __mdok_control('set_next_request', String(name), true); },
  };

  // Legacy `request` global: data of the outgoing request.
  var legacyRequest = {
    url: requestData && requestData.url != null ? String(requestData.url) : '',
    method: requestData && requestData.method ? String(requestData.method).toUpperCase() : 'GET',
    headers: makeHeaderList(requestData && requestData.headers),
    data: requestData && requestData.body && requestData.body.raw != null ? String(requestData.body.raw) : null,
    body: requestBody,
  };
  globalThis.request = legacyRequest;

  // Legacy cookie access.
  globalThis.responseCookies = {
    get: function (name) {
      var key = String(name);
      return Object.prototype.hasOwnProperty.call(cookieMap, key) ? cookieMap[key] : undefined;
    },
    has: function (name) {
      return Object.prototype.hasOwnProperty.call(cookieMap, String(name));
    },
    toObject: function () {
      var out = {};
      for (var k in cookieMap) {
        if (Object.prototype.hasOwnProperty.call(cookieMap, k)) out[k] = cookieMap[k];
      }
      return out;
    },
  };

  /* ------------------------------------------------------------------ *
   * legacy `xml2Json` — xml2js semantics with the official postman-sandbox
   * option set {explicitArray:false, trim:true, mergeAttrs:false}
   * (postmanlabs/postman-sandbox lib/sandbox/xml2Json.js)
   * ------------------------------------------------------------------ */

  function decodeXmlEntities(s) {
    return s.replace(/&(#x?[0-9a-fA-F]+|[a-zA-Z]+);/g, function (m, body) {
      if (body.charAt(0) === '#') {
        var code = body.charAt(1) === 'x' || body.charAt(1) === 'X'
          ? parseInt(body.slice(2), 16)
          : parseInt(body.slice(1), 10);
        if (isNaN(code) || code < 0 || code > 0x10FFFF) return m;
        try { return String.fromCodePoint(code); } catch (e) { return m; }
      }
      switch (body) {
        case 'amp': return '&';
        case 'lt': return '<';
        case 'gt': return '>';
        case 'quot': return '"';
        case 'apos': return "'";
        default: return m;
      }
    });
  }

  function escapeRe(s) {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  // Minimal recursive-descent XML parser producing
  // {name, attrs, children:[...], text}. Comments, PIs, and DOCTYPE are
  // dropped; CDATA and entities become text.
  function parseXmlTree(xml) {
    var pos = 0;
    var len = xml.length;

    function skipWs() {
      while (pos < len && /\s/.test(xml.charAt(pos))) pos++;
    }

    function skipMisc() {
      for (;;) {
        skipWs();
        if (xml.startsWith('<!--', pos)) {
          var cend = xml.indexOf('-->', pos + 4);
          if (cend === -1) { pos = len; return; }
          pos = cend + 3;
        } else if (xml.startsWith('<?', pos)) {
          var pend = xml.indexOf('?>', pos + 2);
          if (pend === -1) { pos = len; return; }
          pos = pend + 2;
        } else if (xml.startsWith('<!DOCTYPE', pos) || xml.startsWith('<!doctype', pos)) {
          var depth = 0;
          var i = pos + 9;
          var dend = -1;
          while (i < len) {
            var ch = xml.charAt(i);
            if (ch === '[') depth++;
            else if (ch === ']') depth--;
            else if (ch === '>' && depth <= 0) { dend = i + 1; break; }
            i++;
          }
          if (dend === -1) { pos = len; return; }
          pos = dend;
        } else {
          break;
        }
      }
    }

    function parseElement() {
      var m = /^<([A-Za-z_][\w.:-]*)/.exec(xml.slice(pos));
      if (!m) return null;
      var name = m[1];
      pos += m[0].length;
      var attrs = {};
      for (;;) {
        skipWs();
        if (xml.startsWith('/>', pos)) {
          pos += 2;
          return { name: name, attrs: attrs, children: [], text: '' };
        }
        if (xml.startsWith('>', pos)) {
          pos += 1;
          break;
        }
        var am = /^([A-Za-z_:][\w.:-]*)\s*=\s*("([^"]*)"|'([^']*)')/.exec(xml.slice(pos));
        if (!am) { pos = len; return null; }
        attrs[am[1]] = decodeXmlEntities(am[3] !== undefined ? am[3] : am[4]);
        pos += am[0].length;
      }
      var children = [];
      var text = '';
      for (;;) {
        if (pos >= len) return null;
        if (xml.startsWith('</', pos)) {
          var closeRe = new RegExp('^</' + escapeRe(name) + '\\s*>');
          var em = closeRe.exec(xml.slice(pos));
          if (!em) return null;
          pos += em[0].length;
          return { name: name, attrs: attrs, children: children, text: text };
        }
        if (xml.startsWith('<!--', pos)) {
          var cend2 = xml.indexOf('-->', pos + 4);
          pos = cend2 === -1 ? len : cend2 + 3;
          continue;
        }
        if (xml.startsWith('<![CDATA[', pos)) {
          var cdend = xml.indexOf(']]>', pos + 9);
          if (cdend === -1) { pos = len; return null; }
          text += xml.slice(pos + 9, cdend);
          pos = cdend + 3;
          continue;
        }
        if (xml.startsWith('<?', pos)) {
          var pend2 = xml.indexOf('?>', pos + 2);
          pos = pend2 === -1 ? len : pend2 + 2;
          continue;
        }
        if (xml.startsWith('<', pos)) {
          var child = parseElement();
          if (!child) return null;
          children.push(child);
          continue;
        }
        var tstart = pos;
        var tend = xml.indexOf('<', tstart);
        if (tend === -1) return null;
        text += decodeXmlEntities(xml.slice(tstart, tend));
        pos = tend;
      }
    }

    skipMisc();
    if (pos >= len || xml.charAt(pos) !== '<') return null;
    var root = parseElement();
    if (!root) return null;
    skipMisc();
    return root;
  }

  // xml2js conversion with explicitArray:false, trim:true, mergeAttrs:false.
  function convertXmlNode(node) {
    var hasAttrs = false;
    for (var a in node.attrs) {
      if (Object.prototype.hasOwnProperty.call(node.attrs, a)) { hasAttrs = true; break; }
    }
    var text = node.text;
    var trimmed = text.trim();
    if (!hasAttrs && node.children.length === 0) {
      return trimmed; // leaf: trimmed text ('' for empty/whitespace-only)
    }
    var obj = {};
    if (hasAttrs) obj.$ = node.attrs;
    if (trimmed !== '') obj._ = trimmed;
    var groups = {};
    for (var j = 0; j < node.children.length; j++) {
      var c = node.children[j];
      if (!groups[c.name]) groups[c.name] = [];
      groups[c.name].push(c);
    }
    for (var gname in groups) {
      if (!Object.prototype.hasOwnProperty.call(groups, gname)) continue;
      var arr = groups[gname];
      obj[gname] = arr.length === 1 ? convertXmlNode(arr[0]) : arr.map(convertXmlNode);
    }
    return obj;
  }

  function xml2Json(xmlString) {
    var root = parseXmlTree(String(xmlString == null ? '' : xmlString));
    if (!root) return {};
    var out = {};
    out[root.name] = convertXmlNode(root);
    return out;
  }
  globalThis.xml2Json = xml2Json;

  /* ------------------------------------------------------------------ *
   * install pm + hardened eval/Function
   * ------------------------------------------------------------------ */

  globalThis.pm = guard('pm', pmRaw);

  globalThis.eval = function () {
    __mdok_eval_used('eval');
    throw new Error('MDOK-PM-EVAL: eval is disabled in the hardened ' + PROFILE + ' profile');
  };
  globalThis.Function = function () {
    __mdok_eval_used('Function');
    throw new Error('MDOK-PM-EVAL: Function is disabled in the hardened ' + PROFILE + ' profile');
  };

  // F1 note: the global-only `eval`/`Function` stubs above are best-effort and
  // are bypassable via the prototype chain in QuickJS (`(function(){}).constructor('...')()`),
  // because QuickJS resolves function `.constructor` to an internal native
  // constructor that is not overwritable from JS. Removing the Eval intrinsic
  // to defeat this at the engine level is not viable here because host-side
  // loading (prelude, user script, vendored modules) goes through Rust
  // `ctx.eval`, which requires the intrinsic. The exploitable half of F1 — the
  // `__mdok_eval_module` global sink that called Rust `ctx.eval` on
  // attacker-controlled source — IS closed by token-gating that host function
  // (pm.rs). The constructor chain can run arbitrary JS only *inside* the
  // sandbox, which has no FS/process access and (since F4) policy-gated network;
  // it is not a host escape. Secrets reachable to such JS are still redacted by
  // the post-run taint sweep (which, since F2, matches encoded forms too).

})();
