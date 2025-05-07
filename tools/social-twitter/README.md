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

- **`ok.id`: [`String`]** - The tweet's unique identifier
- **`ok.text`: [`String`]** - The tweet's content text
- **`ok.author_id`: [`Option<String>`]** - The ID of the tweet's author
- **`ok.created_at`: [`Option<String>`]** - The timestamp when the tweet was created
- **`ok.username`: [`Option<String>`]** - The username of the tweet's author
- **`ok.attachments`: [`Option<Attachments>`]** - Media and polls attached to the tweet
- **`ok.community_id`: [`Option<String>`]** - Community ID if the tweet belongs to a community
- **`ok.context_annotations`: [`Option<Vec<ContextAnnotation>>`]** - Annotations about the tweet content
- **`ok.conversation_id`: [`Option<String>`]** - ID of the conversation this tweet belongs to
- **`ok.edit_controls`: [`Option<EditControls>`]** - Controls for editing the tweet
- **`ok.edit_history_tweet_ids`: [`Option<Vec<String>>`]** - IDs of tweets in the edit history
- **`ok.entities`: [`Option<Entities>`]** - Entities in the tweet (hashtags, mentions, URLs)
- **`ok.geo`: [`Option<Geo>`]** - Geographic information
- **`ok.in_reply_to_user_id`: [`Option<String>`]** - ID of the user being replied to
- **`ok.lang`: [`Option<String>`]** - Language of the tweet
- **`ok.non_public_metrics`: [`Option<NonPublicMetrics>`]** - Private metrics about the tweet
- **`ok.note_tweet`: [`Option<NoteTweet>`]** - Extended note content
- **`ok.organic_metrics`: [`Option<OrganicMetrics>`]** - Organic engagement metrics
- **`ok.possibly_sensitive`: [`Option<bool>`]** - Whether the tweet might contain sensitive content
- **`ok.promoted_metrics`: [`Option<PromotedMetrics>`]** - Metrics from promoted content
- **`ok.public_metrics`: [`Option<PublicMetrics>`]** - Public engagement metrics (likes, retweets, etc.)
- **`ok.referenced_tweets`: [`Option<Vec<ReferencedTweet>>`]** - Tweets referenced by this tweet
- **`ok.reply_settings`: [`Option<String>`]** - Who can reply to this tweet
- **`ok.scopes`: [`Option<Scopes>`]** - Visibility scopes
- **`ok.source`: [`Option<String>`]** - Source of the tweet (client application)
- **`ok.withheld`: [`Option<Withheld>`]** - Withholding information
- **`ok.includes`: [`Option<Includes>`]** - Additional entities related to the tweet:
  - `media`: Images and videos
  - `places`: Geographic locations
  - `polls`: Twitter polls
  - `topics`: Related topics
  - `tweets`: Referenced tweets
  - `users`: Mentioned users
- **`ok.meta`: [`Option<Meta>`]** - Metadata about the tweet request:
  - `newest_id`: Newest tweet ID in a collection
  - `next_token`: Pagination token for next results
  - `oldest_id`: Oldest tweet ID in a collection
  - `previous_token`: Pagination token for previous results
  - `result_count`: Number of results returned

**`err`**

The tweet was not retrieved due to an error.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested tweet or resource was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available. Common codes include:
  - `401` - Unauthorized (authentication error)
  - `403` - Forbidden
  - `404` - Not Found
  - `429` - Too Many Requests (rate limit exceeded)
  - `5xx` - Server errors

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

- **`ok.data`: [`Vec<Tweet>`]** - The collection of tweets from the user's timeline.
- **`ok.includes`: [`Option<Includes>`]** - Additional data included in the response (users, media, polls, etc.)
- **`ok.meta`: [`Option<Meta>`]** - Metadata about the response (result_count, newest_id, oldest_id, next_token, etc.)

**`err`**

The tweets could not be retrieved due to an error.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested tweet or resource was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available. Common codes include:
  - `401` - Unauthorized (authentication error)
  - `403` - Forbidden
  - `404` - Not Found
  - `429` - Too Many Requests (rate limit exceeded)
  - `5xx` - Server errors

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

- **`ok.id`: [`String`]** - The tweet's unique identifier
- **`ok.edit_history_tweet_ids`: [`Vec<String>`]** - List of tweet IDs in the edit history
- **`ok.text`: [`String`]** - The actual content of the tweet

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

- **`ok.data`: [`Vec<Tweet>`]** - The collection of tweets mentioning the specified user.
- **`ok.includes`: [`Option<Includes>`]** - Additional data included in the response (users, media, polls, etc.)
- **`ok.meta`: [`Option<Meta>`]** - Metadata about the response (result_count, newest_id, oldest_id, next_token, etc.)

**`err`**

The tweet mentions retrieval failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested tweet or resource was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available. Common codes include:
  - `401` - Unauthorized (authentication error)
  - `403` - Forbidden
  - `404` - Not Found
  - `429` - Too Many Requests (rate limit exceeded)
  - `5xx` - Server errors

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

- **`ok.id`: [`String`]** - The user's unique identifier
- **`ok.name`: [`String`]** - The user's display name
- **`ok.username`: [`String`]** - The user's @username
- **`ok.protected`: [`Option<bool>`]** - Whether the user's account is protected
- **`ok.affiliation`: [`Option<Affiliation>`]** - The user's affiliation information
- **`ok.connection_status`: [`Option<Vec<ConnectionStatus>>`]** - The user's connection status
- **`ok.created_at`: [`Option<String>`]** - When the user's account was created
- **`ok.description`: [`Option<String>`]** - The user's profile description/bio
- **`ok.entities`: [`Option<Entities>`]** - Entities found in the user's description (hashtags, mentions, URLs)
- **`ok.location`: [`Option<String>`]** - The user's location
- **`ok.most_recent_tweet_id`: [`Option<String>`]** - ID of the user's most recent tweet
- **`ok.pinned_tweet_id`: [`Option<String>`]** - ID of the user's pinned tweet
- **`ok.profile_banner_url`: [`Option<String>`]** - URL of the user's profile banner image
- **`ok.profile_image_url`: [`Option<String>`]** - URL of the user's profile image
- **`ok.public_metrics`: [`Option<PublicMetrics>`]** - Public metrics about the user:
  - `followers_count`: Number of followers
  - `following_count`: Number of accounts the user is following
  - `tweet_count`: Number of tweets the user has posted
  - `listed_count`: Number of lists the user appears on
- **`ok.receives_your_dm`: [`Option<bool>`]** - Whether the user accepts direct messages
- **`ok.subscription_type`: [`Option<SubscriptionType>`]** - The user's subscription type
- **`ok.url`: [`Option<String>`]** - The user's website URL
- **`ok.verified`: [`Option<bool>`]** - Whether the user is verified
- **`ok.verified_type`: [`Option<VerifiedType>`]** - The user's verification type
- **`ok.withheld`: [`Option<Withheld>`]** - Withholding information for the user

**`err`**

The user was not retrieved due to an error.

- **`err.reason`: [`String`]** - Detailed error message describing what went wrong
- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested user was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available. Common codes include:
  - `401` - Unauthorized (authentication error)
  - `403` - Forbidden
  - `404` - Not Found
  - `429` - Too Many Requests (rate limit exceeded)
  - `5xx` - Server errors

It's important to note that some errors may have either a specific error kind (like `NotFound`, `Auth`, or `RateLimit`) or the more general `Api` error kind, and the status code may be a specific value or `None` depending on the error details.

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

- **`ok.id`: [`String`]** - The user's unique identifier
- **`ok.name`: [`String`]** - The user's display name
- **`ok.username`: [`String`]** - The user's @username
- **`ok.protected`: [`Option<bool>`]** - Whether the user's account is protected
- **`ok.affiliation`: [`Option<Affiliation>`]** - The user's affiliation information
- **`ok.connection_status`: [`Option<Vec<ConnectionStatus>>`]** - The user's connection status
- **`ok.created_at`: [`Option<String>`]** - When the user's account was created
- **`ok.description`: [`Option<String>`]** - The user's profile description/bio
- **`ok.entities`: [`Option<Entities>`]** - Entities found in the user's description (hashtags, mentions, URLs)
- **`ok.location`: [`Option<String>`]** - The user's location
- **`ok.most_recent_tweet_id`: [`Option<String>`]** - ID of the user's most recent tweet
- **`ok.pinned_tweet_id`: [`Option<String>`]** - ID of the user's pinned tweet
- **`ok.profile_banner_url`: [`Option<String>`]** - URL of the user's profile banner image
- **`ok.profile_image_url`: [`Option<String>`]** - URL of the user's profile image
- **`ok.public_metrics`: [`Option<PublicMetrics>`]** - Public metrics about the user:
  - `followers_count`: Number of followers
  - `following_count`: Number of accounts the user is following
  - `tweet_count`: Number of tweets the user has posted
  - `listed_count`: Number of lists the user appears on
- **`ok.receives_your_dm`: [`Option<bool>`]** - Whether the user accepts direct messages
- **`ok.subscription_type`: [`Option<SubscriptionType>`]** - The user's subscription type
- **`ok.url`: [`Option<String>`]** - The user's website URL
- **`ok.verified`: [`Option<bool>`]** - Whether the user is verified
- **`ok.verified_type`: [`Option<VerifiedType>`]** - The user's verification type
- **`ok.withheld`: [`Option<Withheld>`]** - Withholding information for the user
- **`ok.includes`: [`Option<Includes>`]** - Additional entities related to the user:
  - `users`: Other users referenced by this user
  - `tweets`: Tweets referenced by this user (e.g., pinned tweet)
  - `media`: Media items referenced by this user
  - `places`: Geographic places referenced by this user
  - `polls`: Polls referenced by this user

**`err`**

The user was not retrieved due to an error.

- **`err.reason`: [`String`]** - Detailed error message describing what went wrong
- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested user was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available. Common codes include:
  - `401` - Unauthorized (authentication error)
  - `403` - Forbidden
  - `404` - Not Found
  - `429` - Too Many Requests (rate limit exceeded)
  - `5xx` - Server errors

It's important to note that some errors may have either a specific error kind (like `NotFound`, `Auth`, or `RateLimit`) or the more general `Api` error kind, and the status code may be a specific value or `None` depending on the error details.

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

- **`ok.id`: [`String`]** - The list's unique identifier
- **`ok.name`: [`String`]** - The list's name
- **`ok.created_at`: [`Option<String>`]** - The timestamp when the list was created
- **`ok.description`: [`Option<String>`]** - The list's description
- **`ok.follower_count`: [`Option<i32>`]** - Number of followers this list has
- **`ok.member_count`: [`Option<i32>`]** - Number of members in this list
- **`ok.owner_id`: [`Option<String>`]** - The ID of the list's owner
- **`ok.private`: [`Option<bool>`]** - Whether the list is private or public
- **`ok.includes`: [`Option<Includes>`]** - Additional entities related to the list
- **`ok.meta`: [`Option<Meta>`]** - Metadata about the list request

**`err`**

The list retrieval failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested list was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available. Common codes include:
  - `401` - Unauthorized (authentication error)
  - `403` - Forbidden
  - `404` - Not Found
  - `429` - Too Many Requests (rate limit exceeded)
  - `5xx` - Server errors

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

- **`ok.data`: [`Option<Vec<Tweet>>`]** - The collection of tweets from the list.
- **`ok.includes`: [`Option<Includes>`]** - Additional data included in the response.
- **`ok.meta`: [`Option<Meta>`]** - Metadata about the response.

**`err`**

The list tweets retrieval failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested list was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available

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

- **`ok.data`: [`Option<Vec<UserData>>`]** - The collection of users who are members of the list.
- **`ok.includes`: [`Option<Includes>`]** - Additional data included in the response.
- **`ok.meta`: [`Option<Meta>`]** - Metadata about the response.

**`err`**

The list members retrieval failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested list was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available

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

- **`ok.is_member`** - Confirmation that the user is a member of the list (true).

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

- **`ok.is_member`** - Confirmation that the user is not a member of the list (false).

**`err`**

The user could not be removed from the list.

- **`err.reason`: [`String`]** - The reason for the error. This could be:
  - Twitter API error
  - Network error
  - Response parsing error
  - Status code error
  - Other error types handled by the centralized error handling mechanism

---

# `xyz.taluslabs.social.twitter.retweet-tweet@1`

Standard Nexus Tool that retweets a specific tweet.
Twitter api [reference](https://docs.x.com/x-api/posts/causes-the-user-in-the-path-to-retweet-the-specified-post)

## Input

**Authentication Parameters**

The following authentication parameters are provided as part of the TwitterAuth structure:

- **`consumer_key`: [`String`]** - Twitter API application's Consumer Key
- **`consumer_secret_key`: [`String`]** - Twitter API application's Consumer Secret Key
- **`access_token`: [`String`]** - Access Token for user's Twitter account
- **`access_token_secret`: [`String`]** - Access Token Secret for user's Twitter account

**Additional Parameters**

**`user_id`: [`String`]**

The ID of the authenticated user who will retweet the tweet.

**`tweet_id`: [`String`]**

The ID of the tweet to retweet.

## Output Variants & Ports

**`ok`**

The tweet was successfully retweeted.

- **`ok.tweet_id`: [`String`]** - The ID of the tweet that was retweeted
- **`ok.retweeted`: [`bool`]** - Confirmation that the tweet was retweeted (true)

**`err`**

The retweet operation failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested tweet was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available. Common codes include:
  - `401` - Unauthorized (authentication error)
  - `403` - Forbidden
  - `404` - Not Found
  - `429` - Too Many Requests (rate limit exceeded)
  - `5xx` - Server errors

It's important to note that some errors may have either a specific error kind (like `NotFound`, `Auth`, or `RateLimit`) or the more general `Api` error kind, and the status code may be a specific value or `None` depending on the error details.

---

# `xyz.taluslabs.social.twitter.get-conversation-messages@1`

Standard Nexus Tool that retrieves direct messages from a conversation. Twitter api [reference](https://developer.twitter.com/en/docs/twitter-api/direct-messages/lookup/api-reference/get-dm_conversations-with-participant_id-dm_events)

## Input

**Authentication Parameters**

The following authentication parameters are provided as part of the TwitterAuth structure:

- **`consumer_key`: [`String`]** - Twitter API application's Consumer Key
- **`consumer_secret_key`: [`String`]** - Twitter API application's Consumer Secret Key
- **`access_token`: [`String`]** - Access Token for user's Twitter account
- **`access_token_secret`: [`String`]** - Access Token Secret for user's Twitter account

**Additional Parameters**

**`participant_id`: [`String`]**

The ID of the participant user for the One to One DM conversation.
Example: "2244994945"

_opt_ **`max_results`: [`Option<i32>`]** _default_: [`100`]

The maximum number of results to return (1-100).

_opt_ **`pagination_token`: [`Option<String>`]** _default_: [`None`]

This parameter is used to get a specified 'page' of results.
Minimum length: 16 characters

_opt_ **`event_types`: [`Option<Vec<String>>`]** _default_: [`None`]

The set of event_types to include in the results.
Available options: MessageCreate, ParticipantsJoin, ParticipantsLeave

_opt_ **`dm_event_fields`: [`Option<Vec<DmEventField>>`]** _default_: [`None`]

A comma separated list of DM Event fields to display.
Available options: attachments, created_at, dm_conversation_id, entities, event_type, id, participant_ids, referenced_tweets, sender_id, text

_opt_ **`expansions`: [`Option<Vec<ExpansionField>>`]** _default_: [`None`]

A comma separated list of fields to expand.
Available options: attachments.media_keys, participant_ids, referenced_tweets.id, sender_id

_opt_ **`media_fields`: [`Option<Vec<MediaField>>`]** _default_: [`None`]

A comma separated list of Media fields to display.
Available options: alt_text, duration_ms, height, media_key, non_public_metrics, organic_metrics, preview_image_url, promoted_metrics, public_metrics, type, url, variants, width

_opt_ **`user_fields`: [`Option<Vec<UserField>>`]** _default_: [`None`]

A comma separated list of User fields to display.
Available options: affiliation, confirmed_email, connection_status, created_at, description, entities, id, is_identity_verified, location, most_recent_tweet_id, name, parody, pinned_tweet_id, profile_banner_url, profile_image_url, protected, public_metrics, receives_your_dm, subscription, subscription_type, url, username, verified, verified_followers_count, verified_type, withheld

_opt_ **`tweet_fields`: [`Option<Vec<TweetField>>`]** _default_: [`None`]

A comma separated list of Tweet fields to display.
Available options: article, attachments, author_id, card_uri, community_id, context_annotations, conversation_id, created_at, display_text_range, edit_controls, edit_history_tweet_ids, entities, geo, id, in_reply_to_user_id, lang, media_metadata, non_public_metrics, note_tweet, organic_metrics, possibly_sensitive, promoted_metrics, public_metrics, referenced_tweets, reply_settings, scopes, source, text, withheld

## Output Variants & Ports

**`ok`**

The direct messages were retrieved successfully.

- **`ok.data`: [`Option<Vec<DmEvent>>`]** - The list of DM events in the conversation
- **`ok.includes`: [`Option<Includes>`]** - Additional information related to the events
- **`ok.meta`: [`Option<Meta>`]** - Pagination metadata with fields such as:
  - `result_count`: Number of results returned
  - `next_token`: Pagination token for next results

**`err`**

The direct messages were not retrieved due to an error.

- **`err.kind`: [`TwitterErrorKind`]** - The type of error that occurred. Possible values:
  - `network` - A network-related error occurred when connecting to Twitter
  - `connection` - Could not establish a connection to Twitter
  - `timeout` - The request to Twitter timed out
  - `parse` - Failed to parse Twitter's response
  - `auth` - Authentication or authorization error
  - `not_found` - The requested conversation or resource was not found
  - `rate_limit` - Twitter's rate limit was exceeded
  - `server` - An error occurred on Twitter's servers
  - `forbidden` - The request was forbidden
  - `api` - An API-specific error occurred
  - `unknown` - An unexpected error occurred
- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.status_code`: [`Option<u16>`]** - The HTTP status code returned by Twitter, if available. Common codes include:
  - `401` - Unauthorized (authentication error)
  - `403` - Forbidden
  - `404` - Not Found
  - `429` - Too Many Requests (rate limit exceeded)
  - `5xx` - Server errors

---

# Error Handling

The Twitter SDK includes a centralized error handling system that provides consistent error responses across all modules. This system includes:

## Error Types (TwitterErrorKind)

The `err.kind` field provides a categorized error type for easier programmatic handling:

- **`network`**: A network-related error occurred when connecting to Twitter
- **`connection`**: Could not establish a connection to Twitter
- **`timeout`**: The request to Twitter timed out
- **`parse`**: Failed to parse Twitter's response
- **`auth`**: Authentication or authorization error
- **`not_found`**: The requested tweet or resource was not found
- **`rate_limit`**: Twitter's rate limit was exceeded
- **`server`**: An error occurred on Twitter's servers
- **`forbidden`**: The request was forbidden
- **`api`**: An API-specific error occurred
- **`unknown`**: An unexpected error occurred

## Error Structure

Each error includes three primary components:

1. **`kind` (TwitterErrorKind)**: The categorized error type (as described above)
2. **`reason` (String)**: A descriptive message that provides details about the error
3. **`status_code` (Option<u16>)**: The HTTP status code returned by Twitter API, if available

### Common Status Codes

- `401`: Unauthorized (authentication error)
- `403`: Forbidden
- `404`: Not Found
- `429`: Too Many Requests (rate limit exceeded)
- `5xx`: Server errors

### Error Message Format

The `reason` field follows a consistent format:

- Network errors: `"Network error: [error details]"`
- Parse errors: `"Response parsing error: [error details]"`
- API errors: `"Twitter API error: [title] (type: [error_type]) - [detail]"`
- Status errors: `"Twitter API status error: [status code]"`
- Other errors: `"Unknown error: [message]"`

## Retryable Errors

Some error types are considered "retryable" and can be attempted again after appropriate backoff:

- `rate_limit`: Consider retrying after the duration specified in the error message
- `network`: Network errors may be temporary and can be retried
- `server`: Server errors (5xx) may be temporary and can be retried

Other error types typically require fixing the request (e.g., `auth`, `not_found`, `forbidden`) and should not be retried without modification.

## Error Handling in Modules

All modules use the `TwitterResult<T>` type for handling errors, which is a type alias for `Result<T, TwitterError>`. This ensures consistent error propagation and formatting throughout the SDK.

The error handling system makes it easier to debug issues with Twitter API calls and provides clear, actionable error messages to end users. The structured error information allows for programmatic handling of specific error conditions.
