# Wake-on-LAN Telegram Bot

A Rust-based Telegram bot for remotely managing servers using Wake-on-LAN (WoL) and SSH.

## Features

- **Wake-on-LAN**: Remotely wake up servers using magic packets
- **Server Management**: Shutdown and status checking via SSH
- **Security**: User authentication with configurable allowed user IDs
- **Inline Keyboards**: Interactive buttons for easy server control
- **Russian Interface**: Полностью русскоязычный интерфейс
- **Confirmation Dialogs**: Safety confirmation for shutdown operations

## Commands

- `/start` - Show the main menu with server control options

## Available Actions

- 🔌 **Включить** - Send Wake-on-LAN magic packet to wake up the server
- 🔴 **Выключить** - Shutdown the server via SSH (with confirmation)
- 🟢 **Статус** - Check server status via SSH connection

## Setup

### Prerequisites

1. **Rust** (latest stable version)
2. **Telegram Bot Token** - Get one from [@BotFather](https://t.me/botfather)
3. **SSH Access** to your router and server
4. **Wake-on-LAN capable network setup**

### Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd wakeonlan_bot
```

2. Build the project:
```bash
cargo build --release
```

### Configuration

Set up the following environment variables:

#### Required Variables

```bash
# Telegram Bot Token from @BotFather
export BOT_TOKEN="your_bot_token_here"

# Comma-separated list of allowed Telegram user IDs
export ALLOWED_USERS="123456789,987654321"

# MAC address of the server to wake up
export SERVER_MAC="aa:bb:cc:dd:ee:ff"
```

#### SSH Configuration (Optional - with defaults)

```bash
# Router SSH settings (for Wake-on-LAN)
export ROUTER_SSH_HOST="localhost"
export ROUTER_SSH_PORT="2223"
export ROUTER_SSH_USER="root"
export ROUTER_SSH_KEY_PATH="/app/keys/id_router_vps_rsa_legacy"

# Server SSH settings (for shutdown/status)
export SERVER_SSH_HOST="localhost"
export SERVER_SSH_PORT="2222"
export SERVER_SSH_USER="friedcerebrum"
export SERVER_SSH_KEY_PATH="/app/keys/id_rsa"

# Timeouts (in seconds)
export SSH_TIMEOUT="10"
export NC_TIMEOUT="3"
```

### Getting User IDs

To find your Telegram user ID:
1. Message [@userinfobot](https://t.me/userinfobot)
2. Add the returned ID to your `ALLOWED_USERS` environment variable

### SSH Key Setup

1. Generate SSH keys for passwordless authentication:
```bash
ssh-keygen -t rsa -b 4096 -f ~/.ssh/id_rsa
```

2. Copy public keys to your router and server:
```bash
ssh-copy-id user@router_ip
ssh-copy-id user@server_ip
```

3. Update the `*_SSH_KEY_PATH` environment variables to point to your private keys

### Wake-on-LAN Setup

1. **Enable WoL on your server's network interface**:
```bash
sudo ethtool -s eth0 wol g
```

2. **Enable WoL in BIOS/UEFI** (varies by motherboard)

3. **Configure router** to support WoL (if using a router to send magic packets)

## Usage

### Running the Bot

```bash
# Set environment variables
export BOT_TOKEN="your_token"
export ALLOWED_USERS="your_user_id"
export SERVER_MAC="server_mac_address"

# Run the bot
cargo run --release
```

### Docker Setup

1. Build the Docker image:
```bash
docker build -t wakeonlan-bot .
```

2. Run with environment variables:
```bash
docker run -d \
  --name wakeonlan-bot \
  -e BOT_TOKEN="your_token" \
  -e ALLOWED_USERS="your_user_id" \
  -e SERVER_MAC="server_mac_address" \
  -v /path/to/ssh/keys:/app/keys:ro \
  wakeonlan-bot
```

### Docker Compose

```yaml
version: '3.8'
services:
  wakeonlan-bot:
    build: .
    environment:
      - BOT_TOKEN=your_bot_token
      - ALLOWED_USERS=123456789,987654321
      - SERVER_MAC=aa:bb:cc:dd:ee:ff
      - ROUTER_SSH_HOST=192.168.1.1
      - SERVER_SSH_HOST=192.168.1.100
    volumes:
      - ./keys:/app/keys:ro
    restart: unless-stopped
```

## Security Considerations

- **User Authentication**: Only users in `ALLOWED_USERS` can control the bot
- **SSH Keys**: Use SSH key authentication instead of passwords
- **Network Security**: Ensure your router and server are properly secured
- **Key Management**: Keep SSH private keys secure and with proper permissions (600)

## Troubleshooting

### Common Issues

1. **"SSH authentication failed"**
   - Check SSH key paths and permissions
   - Verify SSH key is added to target machine's `authorized_keys`
   - Test SSH connection manually

2. **"Magic packet sent but server didn't wake"**
   - Verify WoL is enabled in BIOS
   - Check network interface WoL settings
   - Ensure server is connected via Ethernet (not Wi-Fi)
   - Try waking from same network segment

3. **"Bot doesn't respond"**
   - Check bot token is correct
   - Verify user ID is in ALLOWED_USERS
   - Check bot logs for errors

### Debug Mode

Enable detailed logging:
```bash
RUST_LOG=debug cargo run
```

## Architecture

- **Rust + Tokio**: Async runtime for handling multiple requests
- **Teloxide**: Telegram Bot API framework
- **SSH2**: SSH connections for server management
- **Wake-on-LAN**: Magic packet generation via SSH to router

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [Teloxide](https://github.com/teloxide/teloxide) - Telegram Bot framework
- [SSH2](https://github.com/alexcrichton/ssh2-rs) - SSH client library
- [Tokio](https://tokio.rs/) - Async runtime