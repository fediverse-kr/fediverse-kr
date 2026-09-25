//! Persistence schema only. Domain and HTTP modules do not import this module.
diesel::table! {
    member_legacy_claims (member_id) {
        member_id -> Uuid,
        handle -> Text,
        actor_url -> Nullable<Text>,
        claimed_at -> Nullable<Timestamptz>,
    }
}
diesel::table! {
    member_users (id) {
        id -> Uuid,
        login_id -> Nullable<Text>,
        display_name -> Text,
        password_hash -> Nullable<Text>,
        is_banned -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}
diesel::table! {
    member_sessions (id) {
        id -> Uuid,
        member_id -> Uuid,
        token_hash -> Bytea,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
        authenticated_at -> Timestamptz,
        federated_authenticated_at -> Nullable<Timestamptz>,
    }
}
diesel::table! {
    member_linked_accounts (id) {
        id -> Uuid,
        member_id -> Uuid,
        actor_url -> Text,
        handle -> Text,
        display_name -> Text,
        profile_url -> Text,
        provider -> Text,
        provider_origin -> Text,
        provider_subject_id -> Text,
        is_public -> Bool,
        verified_at -> Timestamptz,
        account_created_at -> Nullable<Timestamptz>,
        account_created_at_verified_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}
diesel::table! {
    member_link_challenges (id) {
        id -> Uuid,
        purpose -> Text,
        member_id -> Nullable<Uuid>,
        session_id -> Nullable<Uuid>,
        browser_binding_hash -> Bytea,
        code_hash -> Bytea,
        actor_url -> Text,
        handle -> Text,
        display_name -> Text,
        profile_url -> Text,
        outbox_url -> Text,
        actor_published_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
        consumed_at -> Nullable<Timestamptz>,
        verified_post_url -> Nullable<Text>,
    }
}
diesel::table! {
    member_auth_rate_limits (scope, key_hash) {
        scope -> Text,
        key_hash -> Bytea,
        attempts -> Int4,
        resets_at -> Timestamptz,
    }
}
diesel::table! {
    federation_instance_keys (singleton) {
        singleton -> Bool,
        private_key_pem -> Text,
        created_at -> Timestamptz,
    }
}
diesel::joinable!(member_sessions -> member_users (member_id));
diesel::table! {
    directory_sites (id) {
        id -> Uuid, domain -> Text, name -> Nullable<Text>, description -> Nullable<Text>,
        is_hidden -> Bool, is_force_hidden -> Bool, is_closed -> Bool,
        created_at -> Timestamptz, updated_at -> Timestamptz,
    }
}
diesel::table! {
    directory_observations (site_id) {
        site_id -> Uuid, is_alive -> Bool, response_time_ms -> Nullable<Int4>,
        status_code -> Nullable<Int4>, health_error -> Nullable<Text>, checked_at -> Timestamptz,
        software -> Nullable<Text>, software_version -> Nullable<Text>, observed_name -> Nullable<Text>,
        observed_description -> Nullable<Text>, registration_open -> Nullable<Bool>,
        user_count -> Nullable<Int8>, active_user_count -> Nullable<Int8>, status_count -> Nullable<Int8>,
        nodeinfo_checked_at -> Nullable<Timestamptz>, nodeinfo_error -> Nullable<Text>, avg_response_time_7d -> Nullable<Int4>,
    }
}
diesel::table! {
    directory_jobs (id) {
        id -> Uuid, site_id -> Uuid, scheduled_at -> Timestamptz, state -> Text,
        attempts -> Int4, lease_token -> Nullable<Uuid>, lease_until -> Nullable<Timestamptz>,
        owner_requested -> Bool,
        last_error -> Nullable<Text>, completed_at -> Nullable<Timestamptz>,
    }
}
diesel::table! {
    directory_health_checks (job_id) {
        job_id -> Uuid, site_id -> Uuid, is_alive -> Bool, response_time_ms -> Nullable<Int4>,
        status_code -> Nullable<Int4>, error -> Nullable<Text>, checked_at -> Timestamptz,
    }
}
diesel::table! {
    directory_icons (site_id) {
        site_id -> Uuid, mime -> Text, bytes -> Bytea, fetched_at -> Timestamptz,
        collection_version -> Int2,
    }
}
diesel::joinable!(directory_icons -> directory_sites (site_id));
diesel::table! {
    directory_site_registrations (site_id) {
        site_id -> Uuid, member_id -> Nullable<Uuid>, created_at -> Timestamptz,
    }
}
diesel::allow_tables_to_appear_in_same_query!(
    directory_sites,
    directory_observations,
    directory_jobs,
    directory_health_checks,
    directory_icons
);
diesel::allow_tables_to_appear_in_same_query!(
    member_users,
    member_sessions,
    member_linked_accounts,
    member_link_challenges,
    member_auth_rate_limits
);
