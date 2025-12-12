use crate::{
    core::{Address, NetworkFamily, PaymentSelection, Resource, Scheme},
    types::Record,
    v1,
};

pub trait PaymentSelector<S: Scheme, A: Address<Network = S::Network>> {
    fn select(scheme: &S, pr: &v1::transport::PaymentRequirements) -> Option<PaymentSelection<A>>;
}

impl<S: Scheme, A: Address<Network = S::Network>> PaymentSelector<S, A> for S {
    fn select(scheme: &S, pr: &v1::transport::PaymentRequirements) -> Option<PaymentSelection<A>> {
        if pr.scheme == S::SCHEME_NAME && pr.network == scheme.network().network_name() {
            Some(PaymentSelection {
                amount: pr.max_amount_required,
                resource: Resource::builder()
                    .url(pr.resource.clone())
                    .description(pr.description.clone())
                    .mime_type(pr.mime_type.clone())
                    .build(),
                pay_to: pr.pay_to.parse().ok()?,
                max_timeout_seconds: pr.max_timeout_seconds,
                asset: pr.asset.parse().ok()?,
                extra: pr.extra.clone(),
                extensions: Record::new(),
            })
        } else {
            None
        }
    }
}
