CREATE TRIGGER audit_accounts_update AFTER UPDATE ON accounts BEGIN
  INSERT INTO audit_logs(id,entity_type,entity_id,action,before_json,after_json)
  VALUES(lower(hex(randomblob(16))),'account',NEW.id,'update',
    json_object('name',OLD.name,'institution',OLD.institution,'type',OLD.account_type,'currency',OLD.base_currency,'active',OLD.is_active),
    json_object('name',NEW.name,'institution',NEW.institution,'type',NEW.account_type,'currency',NEW.base_currency,'active',NEW.is_active));
END;

CREATE TRIGGER audit_instruments_update AFTER UPDATE ON instruments BEGIN
  INSERT INTO audit_logs(id,entity_type,entity_id,action,before_json,after_json)
  VALUES(lower(hex(randomblob(16))),'instrument',NEW.id,'update',
    json_object('symbol',OLD.symbol,'name',OLD.name,'type',OLD.asset_type,'currency',OLD.currency,'active',OLD.is_active),
    json_object('symbol',NEW.symbol,'name',NEW.name,'type',NEW.asset_type,'currency',NEW.currency,'active',NEW.is_active));
END;

CREATE TRIGGER audit_transactions_insert AFTER INSERT ON transactions BEGIN
  INSERT INTO audit_logs(id,entity_type,entity_id,action,after_json)
  VALUES(lower(hex(randomblob(16))),'transaction',NEW.id,
    CASE WHEN NEW.reverses_transaction_id IS NULL THEN 'create' ELSE 'reverse' END,
    json_object('type',NEW.transaction_type,'trade_at',NEW.trade_at,'source',NEW.source,'status',NEW.status,'reverses',NEW.reverses_transaction_id));
END;

CREATE TRIGGER audit_transactions_status AFTER UPDATE OF status ON transactions BEGIN
  INSERT INTO audit_logs(id,entity_type,entity_id,action,before_json,after_json)
  VALUES(lower(hex(randomblob(16))),'transaction',NEW.id,'status_change',json_object('status',OLD.status),json_object('status',NEW.status));
END;

CREATE TRIGGER audit_prices_insert AFTER INSERT ON prices BEGIN
  INSERT INTO audit_logs(id,entity_type,entity_id,action,after_json)
  VALUES(lower(hex(randomblob(16))),'price',NEW.instrument_id,'create',json_object('price_at',NEW.price_at,'price',NEW.price,'currency',NEW.currency,'source',NEW.source));
END;
