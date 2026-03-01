SELECT s.subscriptions_uuid,
    s.users_uuid,
    s.packages_uuid,
    s.created_at AS subscription_created_at,
    s.expired_at,
    s.is_active,
    s.payment_method,
    p.title,
    p.description,
    p.price,
    p.duration_days,
    p.benefits
FROM public.subscriptions s
    JOIN packages p ON p.packages_uuid = s.packages_uuid
WHERE s.users_uuid = $1
ORDER BY s.created_at DESC