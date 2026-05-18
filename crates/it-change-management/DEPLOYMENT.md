# KIAS IT变更管理系统 - 部署指南

## 生产环境部署

### 1. 系统要求

- **操作系统**: RHEL 8/9, Ubuntu 20.04/22.04, Debian 11/12
- **内存**: 最少2GB, 推荐4GB
- **磁盘**: 最少10GB, 推荐50GB
- **数据库**: SQLite (内置) 或 PostgreSQL 14+
- **网络**: 需要访问目标服务器(SSH 22端口)

### 2. 安装步骤

#### 2.1 编译安装

```bash
# 克隆仓库
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS

# 编译
cargo build --release -p kias-it-change-management

# 安装
sudo cp target/release/kias-it-change-management /usr/local/bin/
```

#### 2.2 配置

```bash
# 创建配置目录
sudo mkdir -p /etc/kias/it-change-management

# 创建配置文件
cat > /etc/kias/it-change-management/config.toml << EOF
[server]
host = "0.0.0.0"
port = 8080

[database]
path = "/var/lib/kias/changes.db"

[sla]
critical_hours = 720
high_hours = 336
medium_hours = 168
low_hours = 72
emergency_approval_hours = 72

[linux]
playbook_dir = "/etc/kias/playbooks"
ssh_key_path = "/root/.ssh/id_rsa"
log_dir = "/var/log/kias/compliance"
EOF
```

#### 2.3 创建目录

```bash
# 数据目录
sudo mkdir -p /var/lib/kias
sudo chown kias:kias /var/lib/kias

# 日志目录
sudo mkdir -p /var/log/kias/compliance
sudo chown kias:kias /var/log/kias

# Playbook目录
sudo mkdir -p /etc/kias/playbooks
```

#### 2.4 创建系统用户

```bash
sudo useradd -r -s /bin/false kias
sudo chown -R kias:kias /var/lib/kias /var/log/kias
```

### 3. Systemd服务

```bash
cat > /etc/systemd/system/kias-it-change.service << EOF
[Unit]
Description=KIAS IT Change Management Service
After=network.target

[Service]
Type=simple
User=kias
Group=kias
ExecStart=/usr/local/bin/kias-it-change-management --config /etc/kias/it-change-management/config.toml
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# 启用服务
sudo systemctl daemon-reload
sudo systemctl enable kias-it-change
sudo systemctl start kias-it-change
```

### 4. Nginx反向代理

```nginx
server {
    listen 443 ssl;
    server_name change.example.com;

    ssl_certificate /etc/ssl/certs/change.example.com.crt;
    ssl_certificate_key /etc/ssl/private/change.example.com.key;

    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### 5. 防火墙配置

```bash
# 开放端口
sudo firewall-cmd --permanent --add-port=8080/tcp
sudo firewall-cmd --reload
```

### 6. 备份策略

```bash
# 每日备份脚本
cat > /usr/local/bin/kias-backup.sh << EOF
#!/bin/bash
BACKUP_DIR="/var/backup/kias"
DATE=\$(date +%Y%m%d)
mkdir -p \$BACKUP_DIR
sqlite3 /var/lib/kias/changes.db ".backup \$BACKUP_DIR/changes-\$DATE.db"
find \$BACKUP_DIR -name "*.db" -mtime +30 -delete
EOF

chmod +x /usr/local/bin/kias-backup.sh

# 添加到crontab
echo "0 2 * * * root /usr/local/bin/kias-backup.sh" >> /etc/crontab
```

### 7. 监控

#### 7.1 健康检查

```bash
# 检查服务状态
curl -s http://localhost:8080/health

# 检查数据库连接
curl -s http://localhost:8080/api/v1/stats
```

#### 7.2 日志监控

```bash
# 查看服务日志
journalctl -u kias-it-change -f

# 查看审计日志
sqlite3 /var/lib/kias/changes.db "SELECT * FROM audit_log ORDER BY timestamp DESC LIMIT 100;"
```

### 8. 安全加固

#### 8.1 启用HTTPS

```bash
# Let's Encrypt证书
sudo certbot --nginx -d change.example.com
```

#### 8.2 配置认证

```bash
# 在config.toml中添加
[auth]
enabled = true
method = "ldap"
ldap_url = "ldap://ldap.example.com:389"
ldap_base_dn = "dc=example,dc=com"
```

#### 8.3 审计日志保护

```bash
# 设置审计日志只读
chmod 440 /var/log/kias/compliance/*.log
chown root:root /var/log/kias/compliance/*.log
```

### 9. Linux自动化配置

#### 9.1 Ansible配置

```bash
# 安装Ansible
sudo yum install -y ansible

# 配置SSH密钥
ssh-keygen -t rsa -b 4096 -f /root/.ssh/id_rsa -N ""
ssh-copy-id root@target-server

# 复制Playbooks
cp -r /path/to/KIAS/crates/it-change-management/playbooks/* /etc/kias/playbooks/
```

#### 9.2 OpenSCAP配置

```bash
# 安装OpenSCAP
sudo yum install -y openscap-scanner scap-security-guide

# 下载SCAP内容
sudo yum install -y scap-security-guide
```

### 10. 故障排除

#### 10.1 服务无法启动

```bash
# 检查日志
journalctl -u kias-it-change -n 100

# 检查配置
/usr/local/bin/kias-it-change-management --config /etc/kias/it-change-management/config.toml --check
```

#### 10.2 数据库损坏

```bash
# 检查数据库完整性
sqlite3 /var/lib/kias/changes.db "PRAGMA integrity_check;"

# 从备份恢复
cp /var/backup/kias/changes-YYYYMMDD.db /var/lib/kias/changes.db
```

#### 10.3 性能问题

```bash
# 检查磁盘空间
df -h /var/lib/kias

# 检查数据库大小
du -sh /var/lib/kias/changes.db

# 清理旧数据
sqlite3 /var/lib/kias/changes.db "DELETE FROM audit_log WHERE timestamp < datetime('now', '-365 days');"
```

## 美敦力级别部署清单

- [ ] 系统要求满足
- [ ] 配置文件正确
- [ ] 目录权限正确
- [ ] Systemd服务启用
- [ ] Nginx反向代理配置
- [ ] HTTPS证书安装
- [ ] 防火墙配置
- [ ] 备份策略配置
- [ ] 监控配置
- [ ] 安全加固完成
- [ ] Linux自动化配置
- [ ] 故障排除文档
- [ ] 用户培训完成
- [ ] 验收测试通过
