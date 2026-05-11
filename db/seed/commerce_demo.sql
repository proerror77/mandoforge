INSERT INTO commerce_demo.customers (id, segment, created_at) VALUES
('C-001', 'vip', '2025-12-01'),
('C-002', 'repeat', '2026-01-10'),
('C-003', 'new', '2026-04-15')
ON CONFLICT DO NOTHING;

INSERT INTO commerce_demo.products (sku_id, name, category, brand, unit_cost) VALUES
('SKU-A', 'Everyday Carry Sling', 'bags', 'Mando', 18.00),
('SKU-B', 'Travel Cable Kit', 'accessories', 'Mando', 6.50),
('SKU-D', 'Desk Light Pro', 'home-office', 'Forge', 24.00),
('SKU-F', 'Compact Charger 65W', 'electronics', 'Forge', 12.00),
('SKU-H', 'Noise Filter Mic', 'electronics', 'Forge', 21.00)
ON CONFLICT DO NOTHING;

INSERT INTO commerce_demo.inventory (sku_id, available_qty, reserved_qty, avg_daily_sales) VALUES
('SKU-A', 18, 12, 45),
('SKU-B', 360, 22, 52),
('SKU-D', 128, 9, 31),
('SKU-F', 420, 17, 39),
('SKU-H', 88, 11, 22)
ON CONFLICT (sku_id) DO NOTHING;

INSERT INTO commerce_demo.ad_spend (campaign_id, date, sku_id, spend, attributed_gmv) VALUES
('PAID-SEARCH-1', current_date - 2, 'SKU-F', 2200, 7480),
('PAID-SEARCH-1', current_date - 1, 'SKU-F', 1518, 3610),
('SOCIAL-RET-1', current_date - 2, 'SKU-A', 1200, 5220),
('SOCIAL-RET-1', current_date - 1, 'SKU-A', 980, 1210);

INSERT INTO commerce_demo.tickets (id, sku_id, created_at, category, sentiment) VALUES
('T-001', 'SKU-H', now() - interval '20 hours', 'shipping_delay', 'negative'),
('T-002', 'SKU-D', now() - interval '16 hours', 'quality', 'negative'),
('T-003', 'SKU-A', now() - interval '13 hours', 'stockout', 'negative')
ON CONFLICT DO NOTHING;

INSERT INTO commerce_demo.reviews (id, sku_id, created_at, rating, body) VALUES
('R-001', 'SKU-H', now() - interval '18 hours', 2, 'Arrived late and audio sounded worse than expected'),
('R-002', 'SKU-D', now() - interval '14 hours', 2, 'Lamp flickers after a day'),
('R-003', 'SKU-A', now() - interval '12 hours', 3, 'Wanted black color but it was unavailable')
ON CONFLICT DO NOTHING;

