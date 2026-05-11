CREATE SCHEMA IF NOT EXISTS commerce_demo;

CREATE TABLE IF NOT EXISTS commerce_demo.customers (
    id TEXT PRIMARY KEY,
    segment TEXT NOT NULL,
    created_at DATE NOT NULL
);

CREATE TABLE IF NOT EXISTS commerce_demo.products (
    sku_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    brand TEXT NOT NULL,
    unit_cost NUMERIC NOT NULL
);

CREATE TABLE IF NOT EXISTS commerce_demo.orders (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL REFERENCES commerce_demo.customers(id),
    ordered_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL,
    channel TEXT NOT NULL,
    session_id TEXT
);

CREATE TABLE IF NOT EXISTS commerce_demo.order_items (
    order_id TEXT NOT NULL REFERENCES commerce_demo.orders(id),
    sku_id TEXT NOT NULL REFERENCES commerce_demo.products(sku_id),
    quantity INT NOT NULL,
    unit_price NUMERIC NOT NULL,
    unit_cost NUMERIC NOT NULL
);

CREATE TABLE IF NOT EXISTS commerce_demo.inventory (
    sku_id TEXT PRIMARY KEY REFERENCES commerce_demo.products(sku_id),
    available_qty INT NOT NULL,
    reserved_qty INT NOT NULL,
    avg_daily_sales NUMERIC NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS commerce_demo.ad_spend (
    campaign_id TEXT NOT NULL,
    date DATE NOT NULL,
    sku_id TEXT NOT NULL REFERENCES commerce_demo.products(sku_id),
    spend NUMERIC NOT NULL,
    attributed_gmv NUMERIC NOT NULL
);

CREATE TABLE IF NOT EXISTS commerce_demo.tickets (
    id TEXT PRIMARY KEY,
    sku_id TEXT REFERENCES commerce_demo.products(sku_id),
    created_at TIMESTAMPTZ NOT NULL,
    category TEXT NOT NULL,
    sentiment TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS commerce_demo.reviews (
    id TEXT PRIMARY KEY,
    sku_id TEXT REFERENCES commerce_demo.products(sku_id),
    created_at TIMESTAMPTZ NOT NULL,
    rating INT NOT NULL,
    body TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS commerce_demo.refunds (
    id TEXT PRIMARY KEY,
    order_id TEXT REFERENCES commerce_demo.orders(id),
    sku_id TEXT REFERENCES commerce_demo.products(sku_id),
    amount NUMERIC NOT NULL,
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS commerce_demo.campaigns (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    starts_at TIMESTAMPTZ,
    ends_at TIMESTAMPTZ
);

