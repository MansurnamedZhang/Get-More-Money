CREATE TRIGGER audit_accounts_delete BEFORE DELETE ON accounts BEGIN
  INSERT INTO audit_logs(id,entity_type,entity_id,action,before_json)
  VALUES(lower(hex(randomblob(16))),'account',OLD.id,'delete',
    json_object(
      'name',OLD.name,
      'institution',OLD.institution,
      'type',OLD.account_type,
      'currency',OLD.base_currency,
      'include_in_net_worth',OLD.include_in_net_worth,
      'active',OLD.is_active
    ));
END;
