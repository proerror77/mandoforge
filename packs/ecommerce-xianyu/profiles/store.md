store_profile:
  platform: xianyu
  store_id: required
  seller_id: required
  account_id: required
  operating_mode: draft_first
  buyer_message_policy:
    auto_send_without_approval: false
    max_reply_chars: 300
    manual_handoff_triggers: [refund_dispute, risk_verification, missing_product_evidence, buyer_blacklist_hit]
  delivery_policy:
    auto_send_without_approval: false
    duplicate_send_protection_required: true
    allowed_step_types: [text, image]
