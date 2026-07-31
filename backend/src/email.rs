/// Returns (subject, html_body, plain_text_body)
pub fn otp_email(name: &str, otp: &str) -> (&'static str, String, String) {
    let subject = "Your Cowri verification code";

    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width=device-width,initial-scale=1"/>
<title>Verify your Cowri account</title>
</head>
<body style="margin:0;padding:0;background:#f4f4f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#f4f4f5;padding:40px 16px;">
    <tr><td align="center">
      <table width="100%" style="max-width:480px;background:#ffffff;border-radius:16px;overflow:hidden;box-shadow:0 1px 4px rgba(0,0,0,0.08);">

        <!-- Header -->
        <tr>
          <td style="background:#16a34a;padding:32px 40px;text-align:center;">
            <div style="display:inline-block;background:rgba(255,255,255,0.15);border-radius:12px;padding:10px 20px;">
              <span style="color:#ffffff;font-size:22px;font-weight:700;letter-spacing:-0.5px;">Cowri</span>
            </div>
          </td>
        </tr>

        <!-- Body -->
        <tr>
          <td style="padding:40px 40px 32px;">
            <p style="margin:0 0 8px;font-size:22px;font-weight:700;color:#111827;">Hi {name} 👋</p>
            <p style="margin:0 0 28px;font-size:15px;color:#6b7280;line-height:1.6;">
              Use the code below to verify your email address. It expires in <strong>15 minutes</strong>.
            </p>

            <!-- OTP box -->
            <div style="background:#f0fdf4;border:2px solid #bbf7d0;border-radius:12px;padding:24px;text-align:center;margin-bottom:28px;">
              <p style="margin:0 0 6px;font-size:12px;font-weight:600;color:#16a34a;letter-spacing:1px;text-transform:uppercase;">Verification Code</p>
              <p style="margin:0;font-size:40px;font-weight:800;color:#111827;letter-spacing:10px;">{otp}</p>
            </div>

            <p style="margin:0 0 6px;font-size:13px;color:#9ca3af;line-height:1.6;">
              Enter this code in the Cowri app to complete your registration.
            </p>
            <p style="margin:0;font-size:13px;color:#9ca3af;line-height:1.6;">
              If you didn't create a Cowri account, you can safely ignore this email.
            </p>
          </td>
        </tr>

        <!-- Divider -->
        <tr><td style="padding:0 40px;"><hr style="border:none;border-top:1px solid #f3f4f6;margin:0;"/></td></tr>

        <!-- Footer -->
        <tr>
          <td style="padding:24px 40px;text-align:center;">
            <p style="margin:0 0 4px;font-size:12px;color:#9ca3af;">Save together. Split easy. Pay fast.</p>
            <p style="margin:0;font-size:12px;color:#d1d5db;">© 2026 Cowri · Nigeria</p>
          </td>
        </tr>

      </table>
    </td></tr>
  </table>
</body>
</html>"#, name = name, otp = otp);

    let plain = format!(
        "Hi {name},\n\nYour Cowri verification code is: {otp}\n\nThis code expires in 15 minutes.\n\nIf you didn't create a Cowri account, ignore this email.\n\nCowri — Save together. Split easy. Pay fast.",
        name = name, otp = otp
    );

    (subject, html, plain)
}

pub fn wallet_credited_email(name: &str, amount_kobo: i64, balance_kobo: i64) -> (&'static str, String, String) {
    let amount  = format!("₦{:.2}", amount_kobo  as f64 / 100.0);
    let balance = format!("₦{:.2}", balance_kobo as f64 / 100.0);
    let subject = "Your Cowri wallet has been credited";

    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"/><meta name="viewport" content="width=device-width,initial-scale=1"/></head>
<body style="margin:0;padding:0;background:#f4f4f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#f4f4f5;padding:40px 16px;">
    <tr><td align="center">
      <table width="100%" style="max-width:480px;background:#ffffff;border-radius:16px;overflow:hidden;box-shadow:0 1px 4px rgba(0,0,0,0.08);">
        <tr>
          <td style="background:#16a34a;padding:32px 40px;text-align:center;">
            <span style="color:#ffffff;font-size:22px;font-weight:700;letter-spacing:-0.5px;">Cowri</span>
          </td>
        </tr>
        <tr>
          <td style="padding:40px 40px 32px;">
            <p style="margin:0 0 8px;font-size:22px;font-weight:700;color:#111827;">Money received 🎉</p>
            <p style="margin:0 0 28px;font-size:15px;color:#6b7280;">Hi {name}, your wallet has been credited.</p>

            <div style="background:#f0fdf4;border:2px solid #bbf7d0;border-radius:12px;padding:24px;margin-bottom:28px;">
              <table width="100%" cellpadding="0" cellspacing="0">
                <tr>
                  <td style="font-size:13px;color:#6b7280;">Amount credited</td>
                  <td align="right" style="font-size:20px;font-weight:800;color:#16a34a;">{amount}</td>
                </tr>
                <tr><td colspan="2" style="padding:8px 0;"><hr style="border:none;border-top:1px solid #dcfce7;"/></td></tr>
                <tr>
                  <td style="font-size:13px;color:#6b7280;">New balance</td>
                  <td align="right" style="font-size:15px;font-weight:600;color:#111827;">{balance}</td>
                </tr>
              </table>
            </div>

            <p style="margin:0;font-size:13px;color:#9ca3af;">Open the Cowri app to view your full transaction history.</p>
          </td>
        </tr>
        <tr><td style="padding:0 40px;"><hr style="border:none;border-top:1px solid #f3f4f6;"/></td></tr>
        <tr>
          <td style="padding:24px 40px;text-align:center;">
            <p style="margin:0;font-size:12px;color:#d1d5db;">© 2026 Cowri · Nigeria</p>
          </td>
        </tr>
      </table>
    </td></tr>
  </table>
</body>
</html>"#, name=name, amount=amount, balance=balance);

    let plain = format!("Hi {name},\n\nYour wallet was credited {amount}.\nNew balance: {balance}.\n\nCowri");
    (subject, html, plain)
}

pub fn ajo_payout_email(name: &str, amount_kobo: i64, group_name: &str) -> (&'static str, String, String) {
    let amount  = format!("₦{:.2}", amount_kobo as f64 / 100.0);
    let subject = "Your Ajo payout has arrived";

    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"/><meta name="viewport" content="width=device-width,initial-scale=1"/></head>
<body style="margin:0;padding:0;background:#f4f4f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#f4f4f5;padding:40px 16px;">
    <tr><td align="center">
      <table width="100%" style="max-width:480px;background:#ffffff;border-radius:16px;overflow:hidden;box-shadow:0 1px 4px rgba(0,0,0,0.08);">
        <tr>
          <td style="background:#16a34a;padding:32px 40px;text-align:center;">
            <span style="color:#ffffff;font-size:22px;font-weight:700;letter-spacing:-0.5px;">Cowri</span>
          </td>
        </tr>
        <tr>
          <td style="padding:40px 40px 32px;">
            <p style="margin:0 0 8px;font-size:22px;font-weight:700;color:#111827;">Your Ajo payout is here 🤝</p>
            <p style="margin:0 0 28px;font-size:15px;color:#6b7280;">Hi {name}, it's your turn to collect from <strong>{group_name}</strong>.</p>

            <div style="background:#f0fdf4;border:2px solid #bbf7d0;border-radius:12px;padding:24px;text-align:center;margin-bottom:28px;">
              <p style="margin:0 0 6px;font-size:12px;font-weight:600;color:#16a34a;letter-spacing:1px;text-transform:uppercase;">Payout Amount</p>
              <p style="margin:0;font-size:40px;font-weight:800;color:#111827;">{amount}</p>
            </div>

            <p style="margin:0;font-size:13px;color:#9ca3af;">The money is in your Cowri wallet. Open the app to spend or withdraw.</p>
          </td>
        </tr>
        <tr><td style="padding:0 40px;"><hr style="border:none;border-top:1px solid #f3f4f6;"/></td></tr>
        <tr>
          <td style="padding:24px 40px;text-align:center;">
            <p style="margin:0;font-size:12px;color:#d1d5db;">© 2026 Cowri · Nigeria</p>
          </td>
        </tr>
      </table>
    </td></tr>
  </table>
</body>
</html>"#, name=name, group_name=group_name, amount=amount);

    let plain = format!("Hi {name},\n\nYou received an Ajo payout of {amount} from {group_name}.\n\nCowri");
    (subject, html, plain)
}

pub fn forgot_pin_email(name: &str, otp: &str) -> (&'static str, String, String) {
    let subject = "Reset your Cowri PIN";

    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"/><meta name="viewport" content="width=device-width,initial-scale=1"/></head>
<body style="margin:0;padding:0;background:#f4f4f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#f4f4f5;padding:40px 16px;">
    <tr><td align="center">
      <table width="100%" style="max-width:480px;background:#ffffff;border-radius:16px;overflow:hidden;box-shadow:0 1px 4px rgba(0,0,0,0.08);">
        <tr>
          <td style="background:#16a34a;padding:32px 40px;text-align:center;">
            <span style="color:#ffffff;font-size:22px;font-weight:700;letter-spacing:-0.5px;">Cowri</span>
          </td>
        </tr>
        <tr>
          <td style="padding:40px 40px 32px;">
            <p style="margin:0 0 8px;font-size:22px;font-weight:700;color:#111827;">Reset your PIN 🔐</p>
            <p style="margin:0 0 28px;font-size:15px;color:#6b7280;line-height:1.6;">
              Hi {name}, use the code below to reset your Cowri PIN. It expires in <strong>15 minutes</strong>.
            </p>
            <div style="background:#fff7ed;border:2px solid #fed7aa;border-radius:12px;padding:24px;text-align:center;margin-bottom:28px;">
              <p style="margin:0 0 6px;font-size:12px;font-weight:600;color:#ea580c;letter-spacing:1px;text-transform:uppercase;">Reset Code</p>
              <p style="margin:0;font-size:40px;font-weight:800;color:#111827;letter-spacing:10px;">{otp}</p>
            </div>
            <p style="margin:0 0 8px;font-size:13px;color:#9ca3af;line-height:1.6;">
              If you didn't request a PIN reset, your account is safe — ignore this email.
            </p>
            <p style="margin:0;font-size:13px;color:#ef4444;font-weight:500;">
              Never share this code with anyone, including Cowri support.
            </p>
          </td>
        </tr>
        <tr><td style="padding:0 40px;"><hr style="border:none;border-top:1px solid #f3f4f6;"/></td></tr>
        <tr>
          <td style="padding:24px 40px;text-align:center;">
            <p style="margin:0;font-size:12px;color:#d1d5db;">© 2026 Cowri · Nigeria</p>
          </td>
        </tr>
      </table>
    </td></tr>
  </table>
</body>
</html>"#, name=name, otp=otp);

    let plain = format!(
        "Hi {name},\n\nYour Cowri PIN reset code is: {otp}\n\nExpires in 15 minutes.\n\nNever share this code with anyone.\n\nCowri"
    );
    (subject, html, plain)
}

pub fn bill_paid_email(name: &str, payer_name: &str, amount_kobo: i64, bill_title: &str) -> (&'static str, String, String) {
    let amount  = format!("₦{:.2}", amount_kobo as f64 / 100.0);
    let subject = "Someone paid their share";

    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"/><meta name="viewport" content="width=device-width,initial-scale=1"/></head>
<body style="margin:0;padding:0;background:#f4f4f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#f4f4f5;padding:40px 16px;">
    <tr><td align="center">
      <table width="100%" style="max-width:480px;background:#ffffff;border-radius:16px;overflow:hidden;box-shadow:0 1px 4px rgba(0,0,0,0.08);">
        <tr><td style="background:#16a34a;padding:32px 40px;text-align:center;">
          <span style="color:#ffffff;font-size:22px;font-weight:700;">Cowri</span>
        </td></tr>
        <tr><td style="padding:40px 40px 32px;">
          <p style="margin:0 0 8px;font-size:22px;font-weight:700;color:#111827;">Payment received 🧾</p>
          <p style="margin:0 0 28px;font-size:15px;color:#6b7280;">
            Hi {name}, <strong>{payer_name}</strong> paid their share of <strong>{bill_title}</strong>.
          </p>
          <div style="background:#f0fdf4;border:2px solid #bbf7d0;border-radius:12px;padding:20px;text-align:center;margin-bottom:24px;">
            <p style="margin:0 0 4px;font-size:12px;color:#16a34a;font-weight:600;text-transform:uppercase;letter-spacing:1px;">Amount Received</p>
            <p style="margin:0;font-size:32px;font-weight:800;color:#111827;">{amount}</p>
          </div>
          <p style="margin:0;font-size:13px;color:#9ca3af;">Open Cowri to see the full bill status.</p>
        </td></tr>
        <tr><td style="padding:0 40px;"><hr style="border:none;border-top:1px solid #f3f4f6;"/></td></tr>
        <tr><td style="padding:24px 40px;text-align:center;">
          <p style="margin:0;font-size:12px;color:#d1d5db;">© 2026 Cowri · Nigeria</p>
        </td></tr>
      </table>
    </td></tr>
  </table>
</body>
</html>"#, name=name, payer_name=payer_name, bill_title=bill_title, amount=amount);

    let plain = format!("Hi {name},\n\n{payer_name} paid {amount} for {bill_title}.\n\nCowri");
    (subject, html, plain)
}

pub fn ajo_contribution_email(name: &str, contributor_name: &str, amount_kobo: i64, group_name: &str, cycle: u32) -> (String, String, String) {
    let amount  = format!("₦{:.2}", amount_kobo as f64 / 100.0);
    let subject = format!("New contribution in {group_name}");

    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"/><meta name="viewport" content="width=device-width,initial-scale=1"/></head>
<body style="margin:0;padding:0;background:#f4f4f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#f4f4f5;padding:40px 16px;">
    <tr><td align="center">
      <table width="100%" style="max-width:480px;background:#ffffff;border-radius:16px;overflow:hidden;box-shadow:0 1px 4px rgba(0,0,0,0.08);">
        <tr><td style="background:#16a34a;padding:32px 40px;text-align:center;">
          <span style="color:#ffffff;font-size:22px;font-weight:700;">Cowri</span>
        </td></tr>
        <tr><td style="padding:40px 40px 32px;">
          <p style="margin:0 0 8px;font-size:22px;font-weight:700;color:#111827;">Contribution received 🤝</p>
          <p style="margin:0 0 28px;font-size:15px;color:#6b7280;">
            Hi {name}, <strong>{contributor_name}</strong> contributed to <strong>{group_name}</strong> — Cycle {cycle}.
          </p>
          <div style="background:#f0fdf4;border:2px solid #bbf7d0;border-radius:12px;padding:20px;text-align:center;margin-bottom:24px;">
            <p style="margin:0 0 4px;font-size:12px;color:#16a34a;font-weight:600;text-transform:uppercase;letter-spacing:1px;">Contribution</p>
            <p style="margin:0;font-size:32px;font-weight:800;color:#111827;">{amount}</p>
          </div>
          <p style="margin:0;font-size:13px;color:#9ca3af;">Open Cowri to track your group's progress.</p>
        </td></tr>
        <tr><td style="padding:0 40px;"><hr style="border:none;border-top:1px solid #f3f4f6;"/></td></tr>
        <tr><td style="padding:24px 40px;text-align:center;">
          <p style="margin:0;font-size:12px;color:#d1d5db;">© 2026 Cowri · Nigeria</p>
        </td></tr>
      </table>
    </td></tr>
  </table>
</body>
</html>"#, name=name, contributor_name=contributor_name, group_name=group_name, cycle=cycle, amount=amount);

    let plain = format!("Hi {name},\n\n{contributor_name} contributed {amount} to {group_name} (Cycle {cycle}).\n\nCowri");
    (subject, html, plain)
}
