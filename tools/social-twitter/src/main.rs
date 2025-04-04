//! # `xyz.taluslabs.social.twitter.*`
//!
//! This module contains tools for Twitter operations.
#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;
mod auth;
mod error;
mod list;
mod media;
mod tweet;
mod user;

/// This function bootstraps the tool and starts the server.
#[tokio::main]
async fn main() {
    bootstrap!([
        tweet::post_tweet::PostTweet,
        tweet::get_tweet::GetTweet,
        tweet::like_tweet::LikeTweet,
        tweet::get_mentioned_tweets::GetMentionedTweets,
        tweet::get_user_tweets::GetUserTweets,
        list::create_list::CreateList,
        list::get_list::GetList,
        list::get_list_tweets::GetListTweets,
        list::get_list_members::GetListMembers,
        list::update_list::UpdateList,
        list::add_member::AddMember,
        list::remove_member::RemoveMember,
        media::upload_media::UploadMedia,
        user::get_user_by_id::GetUserById,
        user::get_user_by_username::GetUserByUsername,
    ]);
}
