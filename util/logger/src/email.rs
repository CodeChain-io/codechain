use sendgrid::v3 as sendgrid;

pub struct EmailAlarmConfig {
    pub to: String,
    pub sendgrid_key: String,
    pub network_id: String,
}

impl EmailAlarmConfig {
    pub fn new(to: String, sendgrid_key: String, network_id: String) -> Self {
        Self {
            to,
            sendgrid_key,
            network_id,
        }
    }
}

#[derive(Clone)]
pub struct EmailAlarm {
    pub to: String,
    pub sendgrid_key: String,
    pub network_id: String,
}

impl EmailAlarm {
    pub fn new(config: &EmailAlarmConfig) -> Self {
        Self {
            to: config.to.clone(),
            sendgrid_key: config.sendgrid_key.clone(),
            network_id: config.network_id.clone(),
        }
    }

    pub fn send(&self, log: &str) {
        let p = sendgrid::Personalization::new().add_to(sendgrid::Email::new().set_email(&self.to));
        let now = time::now_utc();
        let now = now.rfc3339();
        let m = sendgrid::Message::new()
            .set_from(sendgrid::Email::new().set_email("no-reply@codechain.io"))
            .set_subject(&format!("[error][{}][codechain] Error from CodeChain-{}", self.network_id, now))
            .add_content(sendgrid::Content::new().set_content_type("text/html").set_value(log))
            .add_personalization(p);
        let sender = sendgrid::Sender::new(self.sendgrid_key.clone());
        let send_result = sender.send(&m);
        if let Err(err) = send_result {
            eprintln!("Sent an email, but failed. returned error is {}", err);
        }
    }
}
