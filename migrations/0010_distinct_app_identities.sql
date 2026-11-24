-- Split previously grouped desktop inventory rows without changing CLI connections.
UPDATE added_applications SET app_id = 'qoder-cn'
WHERE app_id = 'qoder' AND install_path LIKE '%/Qoder CN.app';

UPDATE added_applications SET app_id = 'codebuddy-cn'
WHERE app_id = 'codebuddy' AND install_path LIKE '%/CodeBuddy CN.app';

UPDATE added_applications SET app_id = 'workbuddy'
WHERE app_id = 'codebuddy' AND install_path LIKE '%/WorkBuddy.app';
