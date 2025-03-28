# `xyz.taluslabs.social.twitter.get-tweet@1`

Standard Nexus Tool that retrieves a single tweet from the Twitter API. Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/tweets/lookup/api-reference/get-tweets-id)

## Input

**`bearer_token`: [`String`]**

The bearer token for the user's Twitter account.

**`tweet_id`: [`String`]**

The ID of the tweet to retrieve.

## Output Variants & Ports

**`ok`**

The tweet was retrieved successfully.

- **`ok.data`: [`Option<Tweet>`]** - The tweet data containing:

  - `id`: The tweet's unique identifier
  - `text`: The tweet's content
  - `author_id`: The ID of the tweet's author
  - `created_at`: The timestamp when the tweet was created
  - `username`: The username of the tweet's author
  - And other optional fields like attachments, entities, metrics, etc.

- **`ok.includes`: [`Option<Includes>`]** - Additional data included in the response (users, media, polls, etc.)

- **`ok.meta`: [`Option<Meta>`]** - Metadata about the response

**`err`**

The tweet was not retrieved due to an error.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error with title and error type (e.g., "Twitter API error: Invalid Request (error type: https://api.twitter.com/2/problems/invalid-request)")
  - Twitter API error with optional detail and message (e.g., "Twitter API error: Invalid Request (error type: https://api.twitter.com/2/problems/invalid-request) - One or more parameters to your request was invalid.
  - Failed to parse Twitter API response
  - Failed to read Twitter API response
  - Failed to send request to Twitter API

---

# `xyz.taluslabs.social.twitter.get-user-tweets@1`

Standard Nexus Tool that retrieves tweets from a user's Twitter account. Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/tweets/timelines/api-reference/get-users-id-tweets)

## Input

**`bearer_token`: [`String`]**

The bearer token for the user's Twitter account.

**`user_id`: [`String`]**

The ID of the User to retrieve tweets from.

_opt_ **`since_id`: [`Option<String>`]** _default_: [`None`]

The minimum Post ID to be included in the result set. Takes precedence over start_time if both are specified.

_opt_ **`until_id`: [`Option<String>`]** _default_: [`None`]

The maximum Post ID to be included in the result set. Takes precedence over end_time if both are specified.

_opt_ **`exclude`: [`Option<Vec<ExcludeField>>`]** _default_: [`None`]

The set of entities to exclude (e.g. 'replies' or 'retweets').

_opt_ **`max_results`: [`Option<i32>`]** _default_: [`None`]

The maximum number of results to retrieve (range: 5-100).

_opt_ **`pagination_token`: [`Option<String>`]** _default_: [`None`]

Used to get the next 'page' of results.

_opt_ **`start_time`: [`Option<String>`]** _default_: [`None`]

The earliest UTC timestamp (YYYY-MM-DDTHH:mm:ssZ) from which the Posts will be provided.

_opt_ **`end_time`: [`Option<String>`]** _default_: [`None`]

The latest UTC timestamp (YYYY-MM-DDTHH:mm:ssZ) to which the Posts will be provided.

_opt_ **`tweet_fields`: [`Option<Vec<TweetField>>`]** _default_: [`None`]

A list of Tweet fields to display.

_opt_ **`expansions`: [`Option<Vec<ExpansionField>>`]** _default_: [`None`]

A list of fields to expand.

_opt_ **`media_fields`: [`Option<Vec<MediaField>>`]** _default_: [`None`]

A list of Media fields to display.

_opt_ **`poll_fields`: [`Option<Vec<PollField>>`]** _default_: [`None`]

A list of Poll fields to display.

_opt_ **`user_fields`: [`Option<Vec<UserField>>`]** _default_: [`None`]

A list of User fields to display.

_opt_ **`place_fields`: [`Option<Vec<PlaceField>>`]** _default_: [`None`]

A list of Place fields to display.

## Output Variants & Ports

**`ok`**

The tweets were retrieved successfully.

- **`ok.data`: [`Option<Vec<Tweet>>`]** - The collection of tweets from the user's timeline.
- **`ok.includes`: [`Option<Includes>`]** - Additional data included in the response (users, media, polls, etc.)
- **`ok.meta`: [`Option<Meta>`]** - Metadata about the response (result_count, newest_id, oldest_id, next_token, etc.)

**`err`**

The tweets could not be retrieved due to an error.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error (e.g., "Twitter API error: Not Found Error (type: https://api.twitter.com/2/problems/resource-not-found)")
  - Network error (e.g., "Network error: network error: Connection refused")
  - Response parsing error (e.g., "Response parsing error: expected value at line 1 column 1")
  - Status code error (e.g., "Twitter API status error: 429 Too Many Requests")
  - Other error types handled by the centralized error handling mechanism

---

# `xyz.taluslabs.social.twitter.post-tweet@1`

Standard Nexus Tool that posts a content to Twitter.
Twitter api [reference](https://docs.x.com/x-api/tweets/post-tweet)

## Input

**Authentication Parameters**

The following authentication parameters are provided as part of the TwitterAuth structure:

- **`consumer_key`: [`String`]** - Twitter API application's Consumer Key
- **`consumer_secret_key`: [`String`]** - Twitter API application's Consumer Secret Key
- **`access_token`: [`String`]** - Access Token for user's Twitter account
- **`access_token_secret`: [`String`]** - Access Token Secret for user's Twitter account

**Additional Parameters**

**`text`: [`String`]**

The text content of the tweet.

_opt_ **`card_uri`: [`Option<String>`]** _default_: [`None`]

Card URI for rich media preview. This is mutually exclusive from Quote Tweet ID, Poll, Media, and Direct Message Deep Link.

_opt_ **`community_id`: [`Option<String>`]** _default_: [`None`]

Community ID for community-specific tweets.

_opt_ **`direct_message_deep_link`: [`Option<String>`]** _default_: [`None`]

Direct message deep link. This is mutually exclusive from Quote Tweet ID, Poll, Media, and Card URI.

_opt_ **`for_super_followers_only`: [`Option<bool>`]** _default_: [`None`]

Whether the tweet is for super followers only.

_opt_ **`geo`: [`Option<GeoInfo>`]** _default_: [`None`]

Geo location information containing:

- `place_id`: Place ID for the location

_opt_ **`media`: [`Option<MediaInfo>`]** _default_: [`None`]

Media information containing:

- `media_ids`: List of media IDs to attach (required)
- `tagged_user_ids`: List of user IDs to tag in the media (optional)

This is mutually exclusive from Quote Tweet ID, Poll, and Card URI.

_opt_ **`nullcast`: [`Option<bool>`]** _default_: [`None`]

Whether the tweet should be nullcast (promoted-only). Nullcasted tweets do not appear in the public timeline and are not served to followers.

_opt_ **`poll`: [`Option<PollInfo>`]** _default_: [`None`]

Poll information containing:

- `duration_minutes`: Duration of the poll in minutes (required, range: 5-10080)
- `options`: List of poll options (required, 2-4 options)
- `reply_settings`: Reply settings for the poll (optional)

This is mutually exclusive from Quote Tweet ID, Media, and Card URI.

_opt_ **`quote_tweet_id`: [`Option<String>`]** _default_: [`None`]

ID of the tweet to quote. This is mutually exclusive from Poll, Media, and Card URI.

_opt_ **`reply`: [`Option<ReplyInfo>`]** _default_: [`None`]

Reply information containing:

- `in_reply_to_tweet_id`: ID of the tweet to reply to (required)
- `exclude_reply_user_ids`: List of user IDs to exclude from replies (optional)

_opt_ **`reply_settings`: [`Option<ReplySettings>`]** _default_: [`None`]

Reply settings for the tweet. Can be one of:

- `Following`: Only followers can reply
- `MentionedUsers`: Only mentioned users can reply
- `Subscribers`: Only subscribers can reply

## Output Variants & Ports

**`ok`**

The tweet was posted successfully.

- **`ok.result`: [`TweetResponse`]** - The posted tweet data containing:
  - `id`: The tweet's unique identifier
  - `edit_history_tweet_ids`: List of tweet IDs in the edit history
  - `text`: The actual content of the tweet

**`err`**

The tweet posting failed.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error status (Code/Message format)
  - Twitter API error details (Detail/Status/Title format)
  - Rate limit exceeded (Status: 429)
  - Unauthorized error
  - Invalid JSON response
  - Failed to read Twitter API response
  - Failed to send request to Twitter API
  - Mutually exclusive parameters error (e.g., using both poll and media)
  - "You are not permitted to create an exclusive Tweet" error (when for_super_followers_only is true)

---

# `xyz.taluslabs.social.twitter.like-tweet@1`

Standard Nexus Tool that allows a user to like a specific tweet.
Twitter api [reference](https://docs.x.com/x-api/posts/causes-the-user-in-the-path-to-like-the-specified-post)

## Input

**Authentication Parameters**

The following authentication parameters are provided as part of the TwitterAuth structure:

- **`consumer_key`: [`String`]** - Twitter API application's Consumer Key
- **`consumer_secret_key`: [`String`]** - Twitter API application's Consumer Secret Key
- **`access_token`: [`String`]** - Access Token for user's Twitter account
- **`access_token_secret`: [`String`]** - Access Token Secret for user's Twitter account

**Additional Parameters**

**`user_id`: [`String`]**

The ID of the authenticated user who will like the tweet.

**`tweet_id`: [`String`]**

The ID of the tweet to like.

## Output Variants & Ports

**`ok`**

The tweet was successfully liked.

- **`ok.tweet_id`: [`String`]** - The ID of the tweet that was liked
- **`ok.liked`: [`bool`]** - Confirmation that the tweet was liked (true)

**`err`**

The like operation failed.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error status (Code/Message format)
  - Twitter API error details (Detail/Status/Title format)
  - "You have already liked this Tweet" error
  - Unauthorized error
  - Invalid JSON response
  - Failed to read Twitter API response
  - Failed to send like request to Twitter API

---

# `xyz.taluslabs.social.twitter.get-mentioned-tweets@1`

Standard Nexus Tool that retrieves tweets mentioning a specific user.
Twitter api [reference](https://docs.x.com/x-api/posts/user-mention-timeline-by-user-id)

## Input

**`bearer_token`: [`String`]**

The bearer token for the user's Twitter account.

**`user_id`: [`String`]**

The ID of the User to lookup for mentions.

_opt_ **`since_id`: [`Option<String>`]** _default_: [`None`]

The minimum Post ID to be included in the result set. Takes precedence over start_time if both are specified.

_opt_ **`until_id`: [`Option<String>`]** _default_: [`None`]

The maximum Post ID to be included in the result set. Takes precedence over end_time if both are specified.

_opt_ **`max_results`: [`Option<i32>`]** _default_: [`None`]

The maximum number of results to retrieve (range: 5-100).

_opt_ **`pagination_token`: [`Option<String>`]** _default_: [`None`]

Used to get the next 'page' of results.

_opt_ **`start_time`: [`Option<String>`]** _default_: [`None`]

The earliest UTC timestamp (YYYY-MM-DDTHH:mm:ssZ) from which the Posts will be provided.

_opt_ **`end_time`: [`Option<String>`]** _default_: [`None`]

The latest UTC timestamp (YYYY-MM-DDTHH:mm:ssZ) to which the Posts will be provided.

_opt_ **`tweet_fields`: [`Option<Vec<TweetField>>`]** _default_: [`None`]

A list of Tweet fields to display.

_opt_ **`expansions`: [`Option<Vec<ExpansionField>>`]** _default_: [`None`]

A list of fields to expand.

_opt_ **`media_fields`: [`Option<Vec<MediaField>>`]** _default_: [`None`]

A list of Media fields to display.

_opt_ **`poll_fields`: [`Option<Vec<PollField>>`]** _default_: [`None`]

A list of Poll fields to display.

_opt_ **`user_fields`: [`Option<Vec<UserField>>`]** _default_: [`None`]

A list of User fields to display.

_opt_ **`place_fields`: [`Option<Vec<PlaceField>>`]** _default_: [`None`]

A list of Place fields to display.

## Output Variants & Ports

**`ok`**

The mentioned tweets were retrieved successfully.

- **`ok.data`: [`Option<Vec<Tweet>>`]** - The collection of tweets mentioning the specified user.
- **`ok.includes`: [`Option<Includes>`]** - Additional data included in the response (users, media, polls, etc.)
- **`ok.meta`: [`Option<Meta>`]** - Metadata about the response (result_count, newest_id, oldest_id, next_token, etc.)

**`err`**

The tweet mentions retrieval failed.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error (e.g., "Twitter API error: Not Found Error (type: https://api.twitter.com/2/problems/resource-not-found)")
  - Network error (e.g., "Network error: network error: Connection refused")
  - Response parsing error (e.g., "Response parsing error: expected value at line 1 column 1")
  - Status code error (e.g., "Twitter API status error: 429 Too Many Requests")
  - Other error types handled by the centralized error handling mechanism

---

# `xyz.taluslabs.social.twitter.get-user-by-id@1`

Standard Nexus Tool that retrieves a user from the Twitter API by their ID. Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/users/lookup/api-reference/get-users-id)

## Input

**`bearer_token`: [`String`]**

The bearer token for the user's Twitter account.

**`user_id`: [`String`]**

The ID of the User to lookup (e.g. "2244994945").

_opt_ **`user_fields`: [`Option<Vec<UserField>>`]** _default_: [`None`]

A comma separated list of User fields to display.

_opt_ **`expansions_fields`: [`Option<Vec<ExpansionField>>`]** _default_: [`None`]

A comma separated list of fields to expand.

_opt_ **`tweet_fields`: [`Option<Vec<TweetField>>`]** _default_: [`None`]

A comma separated list of Tweet fields to display.

## Output Variants & Ports

**`ok`**

The user was retrieved successfully.

- **`ok.result`: [`UserResponse`]** - The user data containing all fields from the Twitter API response.

**`err`**

The user was not retrieved due to an error.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error status
  - User not found
  - Invalid token
  - Rate limit exceeded
  - Failed to parse Twitter API response
  - Failed to read Twitter API response
  - Failed to send request to Twitter API

---

# `xyz.taluslabs.social.twitter.get-user-by-username@1`

Standard Nexus Tool that retrieves a user from the Twitter API by username. Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/users/lookup/api-reference/get-users-by-username-username)

## Input

**`bearer_token`: [`String`]**

The bearer token for the user's Twitter account.

**`username`: [`String`]**

The username to retrieve (without the @ symbol).

_opt_ **`user_fields`: [`Option<Vec<UserField>>`]** _default_: [`None`]

A list of User fields to display.

_opt_ **`expansions_fields`: [`Option<Vec<ExpansionField>>`]** _default_: [`None`]

A list of fields to expand.

_opt_ **`tweet_fields`: [`Option<Vec<TweetField>>`]** _default_: [`None`]

A list of Tweet fields to display.

## Output Variants & Ports

**`ok`**

The user was retrieved successfully.

- **`ok.result`: [`UserResponse`]** - The user data containing all fields from the Twitter API response.

**`err`**

The user was not retrieved due to an error.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error status
  - User not found
  - Invalid token
  - Rate limit exceeded
  - Failed to parse Twitter API response
  - Failed to read Twitter API response
  - Failed to send request to Twitter API

---

# `xyz.taluslabs.social.twitter.create-list@1`

Standard Nexus Tool that creates a new list on Twitter.
Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/lists/manage-lists/api-reference/post-lists)

## Input

**Authentication Parameters**

The following authentication parameters are provided as part of the TwitterAuth structure:

- **`consumer_key`: [`String`]** - Twitter API application's Consumer Key
- **`consumer_secret_key`: [`String`]** - Twitter API application's Consumer Secret Key
- **`access_token`: [`String`]** - Access Token for user's Twitter account
- **`access_token_secret`: [`String`]** - Access Token Secret for user's Twitter account

**Additional Parameters**

**`name`: [`String`]**

The name of the list.

_opt_ **`description`: [`Option<String>`]** _default_: [`None`]

Description of the list.

_opt_ **`private`: [`Option<bool>`]** _default_: [`None`]

Determines if the list is private (true) or public (false).

## Output Variants & Ports

**`ok`**

The list was created successfully.

- **`ok.result`** - The created list data containing:
  - `id`: The list's unique identifier
  - `name`: The name of the list
  - And other details like description, member count, follower count, etc.

**`err`**

The list creation failed.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error (e.g., "Twitter API error: Not Found Error (type: https://api.twitter.com/2/problems/resource-not-found)")
  - Network error (e.g., "Network error: network error: Connection refused")
  - Response parsing error
  - Status code error
  - Other error types handled by the centralized error handling mechanism

---

# `xyz.taluslabs.social.twitter.get-list@1`

Standard Nexus Tool that retrieves a list from Twitter by ID.
Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/lists/list-lookup/api-reference/get-lists-id)

## Input

**`bearer_token`: [`String`]**

The bearer token for the user's Twitter account.

**`list_id`: [`String`]**

The ID of the list to retrieve.

_opt_ **`list_fields`: [`Option<Vec<ListField>>`]** _default_: [`None`]

A list of List fields to display.

_opt_ **`expansions`: [`Option<Vec<ExpansionField>>`]** _default_: [`None`]

A list of fields to expand.

_opt_ **`user_fields`: [`Option<Vec<UserField>>`]** _default_: [`None`]

A list of User fields to display.

## Output Variants & Ports

**`ok`**

The list was retrieved successfully.

- **`ok.data`** - The list data.
- **`ok.includes`** - Additional data included in the response.

**`err`**

The list retrieval failed.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error
  - Network error
  - Response parsing error
  - Status code error
  - Other error types handled by the centralized error handling mechanism

---

# `xyz.taluslabs.social.twitter.get-list-tweets@1`

Standard Nexus Tool that retrieves tweets from a Twitter list.
Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/lists/list-tweets/api-reference/get-lists-id-tweets)

## Input

**`bearer_token`: [`String`]**

The bearer token for the user's Twitter account.

**`list_id`: [`String`]**

The ID of the list to retrieve tweets from.

_opt_ **`max_results`: [`Option<i32>`]** _default_: [`None`]

The maximum number of results to retrieve (range: 5-100).

_opt_ **`pagination_token`: [`Option<String>`]** _default_: [`None`]

Used to get the next 'page' of results.

_opt_ **`tweet_fields`: [`Option<Vec<TweetField>>`]** _default_: [`None`]

A list of Tweet fields to display.

_opt_ **`expansions`: [`Option<Vec<ExpansionField>>`]** _default_: [`None`]

A list of fields to expand.

_opt_ **`media_fields`: [`Option<Vec<MediaField>>`]** _default_: [`None`]

A list of Media fields to display.

_opt_ **`poll_fields`: [`Option<Vec<PollField>>`]** _default_: [`None`]

A list of Poll fields to display.

_opt_ **`user_fields`: [`Option<Vec<UserField>>`]** _default_: [`None`]

A list of User fields to display.

_opt_ **`place_fields`: [`Option<Vec<PlaceField>>`]** _default_: [`None`]

A list of Place fields to display.

## Output Variants & Ports

**`ok`**

The list tweets were retrieved successfully.

- **`ok.data`** - The collection of tweets from the list.
- **`ok.includes`** - Additional data included in the response.
- **`ok.meta`** - Metadata about the response.

**`err`**

The list tweets retrieval failed.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error
  - Network error
  - Response parsing error
  - Status code error
  - Other error types handled by the centralized error handling mechanism

---

# `xyz.taluslabs.social.twitter.get-list-members@1`

Standard Nexus Tool that retrieves members of a Twitter list.
Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/lists/list-members/api-reference/get-lists-id-members)

## Input

**`bearer_token`: [`String`]**

The bearer token for the user's Twitter account.

**`list_id`: [`String`]**

The ID of the list to retrieve members from.

_opt_ **`max_results`: [`Option<i32>`]** _default_: [`None`]

The maximum number of results to retrieve (range: 1-100).

_opt_ **`pagination_token`: [`Option<String>`]** _default_: [`None`]

Used to get the next 'page' of results.

_opt_ **`user_fields`: [`Option<Vec<UserField>>`]** _default_: [`None`]

A list of User fields to display.

_opt_ **`expansions`: [`Option<Vec<ExpansionField>>`]** _default_: [`None`]

A list of fields to expand.

_opt_ **`tweet_fields`: [`Option<Vec<TweetField>>`]** _default_: [`None`]

A list of Tweet fields to display.

## Output Variants & Ports

**`ok`**

The list members were retrieved successfully.

- **`ok.data`** - The collection of users who are members of the list.
- **`ok.includes`** - Additional data included in the response.
- **`ok.meta`** - Metadata about the response.

**`err`**

The list members retrieval failed.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error
  - Network error
  - Response parsing error
  - Status code error
  - Other error types handled by the centralized error handling mechanism

---

# `xyz.taluslabs.social.twitter.update-list@1`

Standard Nexus Tool that updates an existing Twitter list.
Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/lists/manage-lists/api-reference/put-lists-id)

## Input

**Authentication Parameters**

The following authentication parameters are provided as part of the TwitterAuth structure:

- **`consumer_key`: [`String`]** - Twitter API application's Consumer Key
- **`consumer_secret_key`: [`String`]** - Twitter API application's Consumer Secret Key
- **`access_token`: [`String`]** - Access Token for user's Twitter account
- **`access_token_secret`: [`String`]** - Access Token Secret for user's Twitter account

**Additional Parameters**

**`list_id`: [`String`]**

The ID of the list to update.

_opt_ **`name`: [`Option<String>`]** _default_: [`None`]

The new name for the list.

_opt_ **`description`: [`Option<String>`]** _default_: [`None`]

The new description for the list.

_opt_ **`private`: [`Option<bool>`]** _default_: [`None`]

Whether the list should be private (true) or public (false).

## Output Variants & Ports

**`ok`**

The list was updated successfully.

- **`ok.list_id`** - The ID of the updated list.
- **`ok.updated`** - Confirmation that the list was updated (true).

**`err`**

The list update failed.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error
  - Network error
  - Response parsing error
  - Status code error
  - Other error types handled by the centralized error handling mechanism

---

# `xyz.taluslabs.social.twitter.add-member@1`

Standard Nexus Tool that adds a user to a Twitter list.
Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/lists/list-members/api-reference/post-lists-id-members)

## Input

**Authentication Parameters**

The following authentication parameters are provided as part of the TwitterAuth structure:

- **`consumer_key`: [`String`]** - Twitter API application's Consumer Key
- **`consumer_secret_key`: [`String`]** - Twitter API application's Consumer Secret Key
- **`access_token`: [`String`]** - Access Token for user's Twitter account
- **`access_token_secret`: [`String`]** - Access Token Secret for user's Twitter account

**Additional Parameters**

**`list_id`: [`String`]**

The ID of the list to which a member will be added.

**`user_id`: [`String`]**

The ID of the user to add to the list.

## Output Variants & Ports

**`ok`**

The user was successfully added to the list.

- **`ok.list_id`** - The ID of the list.
- **`ok.user_id`** - The ID of the user who was added to the list.
- **`ok.added`** - Confirmation that the user was added to the list (true).

**`err`**

The user could not be added to the list.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error
  - Network error
  - Response parsing error
  - Status code error
  - Other error types handled by the centralized error handling mechanism

---

# `xyz.taluslabs.social.twitter.remove-member@1`

Standard Nexus Tool that removes a user from a Twitter list.
Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/lists/list-members/api-reference/delete-lists-id-members-user_id)

## Input

**Authentication Parameters**

The following authentication parameters are provided as part of the TwitterAuth structure:

- **`consumer_key`: [`String`]** - Twitter API application's Consumer Key
- **`consumer_secret_key`: [`String`]** - Twitter API application's Consumer Secret Key
- **`access_token`: [`String`]** - Access Token for user's Twitter account
- **`access_token_secret`: [`String`]** - Access Token Secret for user's Twitter account

**Additional Parameters**

**`list_id`: [`String`]**

The ID of the list from which a member will be removed.

**`user_id`: [`String`]**

The ID of the user to remove from the list.

## Output Variants & Ports

**`ok`**

The user was successfully removed from the list.

- **`ok.list_id`** - The ID of the list.
- **`ok.user_id`** - The ID of the user who was removed from the list.
- **`ok.removed`** - Confirmation that the user was removed from the list (true).

**`err`**

The user could not be removed from the list.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error
  - Network error
  - Response parsing error
  - Status code error
  - Other error types handled by the centralized error handling mechanism

---

# Error Handling

The Twitter SDK includes a centralized error handling system that provides consistent error responses across all modules. This system includes:

## Error Types

- **Network Errors**: Errors that occur during network communication with the Twitter API.
- **Parse Errors**: Errors that occur when parsing the Twitter API response JSON.
- **API Errors**: Errors returned by the Twitter API with specific titles, types, and details.
- **Status Errors**: HTTP status errors from the Twitter API.
- **Other Errors**: Any other errors that don't fit into the above categories.

## Error Structure

Each error includes a descriptive message that follows a consistent format:

- Network errors: `"Network error: [error details]"`
- Parse errors: `"Response parsing error: [error details]"`
- API errors: `"Twitter API error: [title] (type: [error_type]) - [detail]"`
- Status errors: `"Twitter API status error: [status code]"`
- Other errors: `"Unknown error: [message]"`

## Error Handling in Modules

All modules use the `TwitterResult<T>` type for handling errors, which is a type alias for `Result<T, TwitterError>`. This ensures consistent error propagation and formatting throughout the SDK.

The error handling system makes it easier to debug issues with Twitter API calls and provides clear, actionable error messages to end users.
