# Sanasana

A fintech web application for Nigerians — digital rotating savings (Ajo/Esusu), bill splitting, and wallet management with Paystack payments.

Built with Rust across the full stack: REST API on the backend, WebAssembly on the frontend.

---

## Features

- **Wallet** — fund via Paystack, track balance and transaction history
- **Ajo Groups** — create and manage digital rotating savings circles with automatic contribution and payout logic
- **Bill Splitting** — split expenses among participants by phone number, settle shares from wallet
- **Auth** — phone + PIN registration and login with JWT sessions
- **PWA** — installable progressive web app with offline support via service worker

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, GlideAPI, Tokio |
| Frontend | Rust, Leptos, WebAssembly |
| Payments | Paystack |
| Styling | Tailwind CSS |

---

## Project Structure

```
sanasana/
├── shared/       # Shared types and DTOs (User, Wallet, AjoGroup, Bill)
├── backend/      # REST API
│   └── src/
│       ├── services/   # auth, wallet, ajo, bills
│       ├── routes/     # HTTP handlers
│       ├── store/      # In-memory store (swap for Postgres in production)
│       └── middleware.rs
└── frontend/     # Leptos WASM PWA
    └── src/
        ├── pages/      # auth, dashboard
        └── components/ # Button, Card, Input, Badge
```

---

## Getting Started

### Prerequisites

- Rust (stable)
- `wasm32-unknown-unknown` target
- [Trunk](https://trunkrs.dev) for frontend builds

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
```

### Environment

Create `backend/.env`:

```env
PAYSTACK_SECRET_KEY=sk_test_your_key_here
```

### Run

```bash
# Backend (port 3000)
cargo run -p backend

# Frontend (port 8080)
cd frontend && trunk serve
```

---

## API Reference

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/auth/register` | Register with name, phone, PIN |
| POST | `/auth/login` | Login, returns JWT |
| GET | `/wallet` | Get wallet balance |
| GET | `/wallet/transactions` | Transaction history |
| POST | `/wallet/fund` | Initialize Paystack top-up |
| POST | `/webhook/paystack` | Paystack webhook receiver |
| GET | `/ajo` | List user's ajo groups |
| POST | `/ajo` | Create a new ajo group |
| POST | `/ajo/:id/join` | Join an existing group |
| POST | `/ajo/:id/contribute` | Contribute for current cycle |
| GET | `/bills` | List user's bills |
| POST | `/bills` | Create and split a bill |
| POST | `/bills/:id/pay` | Pay your share of a bill |

All authenticated endpoints require `Authorization: Bearer <token>`.

---

## Paystack Webhook

For local development, expose the backend with [ngrok](https://ngrok.com):

```bash
ngrok http 3000
```

Set `https://<your-ngrok-url>/webhook/paystack` as the webhook URL in your Paystack dashboard.

---

## License

MIT
