---
title: Onboarding
sidebar_position: 2
---

This document primarily serves as a guide for new committers to Apache OpenDAL™.

Upon receiving an invitation email from OpenDAL PMC, a new committer should consider whether accepting.
If they decide in favor, they should select "Reply All" and express their decision.

## Submit CLA

1. Download the ICLA from https://www.apache.org/licenses/contributor-agreements.html#clas. If a corporation assigns employees to work on an Apache project, please download the CCLA.
2. Complete the ICLA based on your particulars. Please note:
    - The address field should be filled out accurately and in detail.
    - You need to choose a unique ApacheID that hasn't been taken. Check https://people.apache.org/committer-index.html to see which IDs are still available.
3. Sign the document by hand or by electronic signature
    - Manually sign a printed copy, then scan it to produce a pdf.
    - Digitally draw a signature on the document: [Detail Instruction](https://www.apache.org/licenses/cla-faq.html#printer).
    - Sign the document using PGP: [Detail Instruction](https://www.apache.org/licenses/contributor-agreements.html#submitting).
4. Send your icla.pdf (and icla.pdf.asc if PGP-signed) to [secretary@apache.org](mailto:secretary@apache.org).

After waiting for some time, you will receive an email notifying you that your CLA has been successfully recorded.

## Setup ASF Account

When receiving an email with the subject "Welcome to the Apache Software Foundation" from root@apache.org, we can begin setting up an ASF account.

### Setup LDAP Password

1. Go to https://id.apache.org/reset/enter and enter your ApacheID.
2. Check your email and click the provided link to reset your password.

### Link ASF Account to GitHub

1. Navigate to https://gitbox.apache.org/boxer/ and enter your ApacheID and password.
2. Click "Authenticate with GitHub" and follow the given instructions to link your ASF account to GitHub.
3. Check your email titled "[GitHub] @asfgit has invited you to join the @apache organization" and accept the invitation.
4. Wait momentarily, and the website will refresh on its own.
5. (If you do not enable 2FA on GitHub) Please follow the [instruction](https://docs.github.com/en/authentication/securing-your-account-with-two-factor-authentication-2fa/configuring-two-factor-authentication).

Your ApacheID and GitHub ID will now both appear on https://gitbox.apache.org/boxer/. Congrats on successfully linking your ASF account to GitHub!

## Email Settings

Note: Apache does not provide a mailbox directly.

### Receive Email

You can change your forwarding email address at [Apache Account Utility Platform](https://id.apache.org/)

### Send Email 

To send emails using your apache.org address, configure your email client to leverage the `mail-relay` service. For specifics, refer to [this guide](https://infra.apache.org/committer-email.html).

Here's an illustration for Gmail users:

1. Open Gmail settings and select "See all settings".
2. Navigate to "Accounts and Import", then locate "Send mail as".
3. Click "Add another email address" and enter your name and apache.org email address.
4. Input the SMTP server information:
    - SMTP Server: mail-relay.apache.org
    - Port: 587
    - Username: your apacheID
    - Password: your apacheID password
    - Secured connection using TLS
5. Click "Add account" and you will receive an email from Gmail that need to confirm.

![Gmail Settings](gmail-smtp-setting.jpg)

### Subscribe to Mailing List

1. Send email to [dev-subscribe@opendal.apache.org](mailto:dev-subscribe@opendal.apache.org)
2. You will receive an email with the subject "confirm subscribe to dev@opendal.apache.org"
3. Reply to the email with "Confirm" in the body

If you receive an email with the subject "WELCOME to dev@opendal.apache.org", you have successfully subscribed to the mailing list.

## Setup 1password

OpenDAL offers a 1Password open-source team license for conducting local integration tests (Thanks to 1Password!). Once you have been added to OpenDAL's committer list, one of the PMC members will invite you to join the team.

OpenDAL committers can also self-register to join the OpenDAL 1Password team by visiting <https://opendal.1password.com/teamjoin/invitation/DHRDX3DG45HDTNMYK63BK5BTQM> and completing the signup with their `apache.org` email address.

Please download your preferred clients to begin using it. You can create your own vault that is accessible only by yourself. Neither the 1password team nor OpenDAL PMC members can access it unless you choose to share it.
