# authsrv

Demo available at https://trihard.online

API
===

### Sign up (email + password)

`POST /signup`

**Content example**

```json
{
  "login": "user@example.com",
  "secret": "mypassword"
}
```

**Responses**

Signed up: `200 OK`

```json
"Ok"
```

User already exists: `200 OK`

```json
{
  "UserAlreadyExists": "user@example.com"
}
```

### Sign in (email + password)

`POST /signin`

**Content example**

```json
{
  "login": "user@example.com",
  "secret": "mypassword"
}
```

**Responses**

Signed in: `200 OK`

Returns internal authentication token. *Authentication token remains valid for 60 seconds*

```json
{
  "Token": "91d111de-347a-42b5-9ae7-e752a63b4767"
}
```

Invalid login/password: `200 OK`

```json
{
  "InvalidCredentials": "user@example.com"
}
```

### Sign up (Facebook user access token)

`POST /fb/signup`

**Content example**

```json
{
  "fb_token": "<facebook token>"
}
```

**Responses**

Signed up: `200 OK`

```json
"Ok"
```

Invalid Facebook token: `404 Not Found`

Could not retrieve Facebook user ID: `404 Not Found`

### Sign in (Facebook user access token)

`POST /fb/signin`

**Content example**

```json
{
  "fb_token": "<facebook token>"
}
```

**Responses**

Signed in: `200 OK`

Returns internal authentication token. *Authentication token remains valid for 60 seconds*

```json
{
  "Token": "91d111de-347a-42b5-9ae7-e752a63b4767"
}
```

Invalid Facebook token: `404 Not Found`

Could not retrieve Facebook user ID: `404 Not Found`

### Check if authenticated

`GET /test_auth/{auth_token}`

**Responses**

Authenticated: `200 OK`

Response for email auth:

```json
user@example.com
```

Response for Facebook token auth (returns Facebook user ID):

```json
1234567890
```

Invalid or expired token: `404 Not Found`
