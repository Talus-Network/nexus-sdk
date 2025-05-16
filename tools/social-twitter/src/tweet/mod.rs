//! # `xyz.taluslabs.social.twitter.*`
//!
//! This module contains tools for Twitter operations.

pub(crate) mod get_mentioned_tweets;
pub(crate) mod get_recent_search_tweets;
pub(crate) mod get_tweet;
pub(crate) mod get_user_tweets;
pub(crate) mod like_tweet;
pub(crate) mod models;
pub(crate) mod post_tweet;
pub(crate) mod retweet_tweet;

pub const TWITTER_API_BASE: &str = "https://api.twitter.com/2";
