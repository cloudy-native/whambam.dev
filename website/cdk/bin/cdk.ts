#!/opt/homebrew/opt/node/bin/node
import * as cdk from 'aws-cdk-lib';
import { CdkStack } from '../lib/cdk-stack';

const app = new cdk.App();
const env: cdk.Environment = {
  account: process.env.CDK_DEFAULT_ACCOUNT,
  region: process.env.CDK_DEFAULT_REGION,
};
new CdkStack(app, 'whambam-website', {
  env,
  domainName: 'whambam.dev',
});