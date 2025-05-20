//! # `xyz.taluslabs.social.twitter.*`
//!
//! This module contains tools for Twitter operations.
#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;
mod auth;
mod direct_message;
mod error;
mod list;
mod tweet;
mod twitter_client;
mod user;

/// This function bootstraps the tool and starts the server.
#[tokio::main]
async fn main() {
    bootstrap!([
        tweet::post_tweet::PostTweet,
        tweet::delete_tweet::DeleteTweet,
        tweet::get_tweet::GetTweet,
        tweet::like_tweet::LikeTweet,
        tweet::get_mentioned_tweets::GetMentionedTweets,
        tweet::get_user_tweets::GetUserTweets,
        tweet::get_recent_tweet_count::GetRecentTweetCount,
        tweet::get_recent_search_tweets::GetRecentSearchTweets,
        tweet::unlike_tweet::UnlikeTweet,
        tweet::undo_retweet_tweet::UndoRetweetTweet,
        tweet::get_tweets::GetTweets,
        tweet::retweet_tweet::RetweetTweet,
        list::create_list::CreateList,
        list::delete_list::DeleteList,
        list::get_list::GetList,
        list::get_list_tweets::GetListTweets,
        list::get_list_members::GetListMembers,
        list::update_list::UpdateList,
        list::add_member::AddMember,
        list::get_user_lists::GetUserLists,
        list::remove_member::RemoveMember,
        user::get_user_by_id::GetUserById,
        user::get_user_by_username::GetUserByUsername,
        direct_message::get_conversation_messages_by_id::GetConversationMessagesById,
        direct_message::get_conversation_messages::GetConversationMessages,
        direct_message::send_direct_message::SendDirectMessage,
        user::unfollow_user::UnfollowUser,
        user::get_users_by_username::GetUsersByUsername,
        user::get_users_by_id::GetUsersById,
    ]);
}
