INSERT INTO public.subscriptions(
        users_uuid,
        packages_uuid,
        created_at,
        expired_at,
        is_active,
        payment_method
    )
VALUES ($1, $2, now(), $3, true, $4)
RETURNING subscriptions_uuid