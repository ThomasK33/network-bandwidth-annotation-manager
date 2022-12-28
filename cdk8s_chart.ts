import { Chart, ChartProps } from "cdk8s";
import { Construct } from "constructs";
import {
  Certificate,
  CertificateSpecPrivateKeyAlgorithm,
  CertificateSpecPrivateKeyEncoding,
  CertificateSpecUsages,
  ClusterIssuer,
} from "../../../imports/cert-manager.io";
import {
  KubeDeployment,
  KubeMutatingWebhookConfiguration,
  KubeNamespace,
  KubeService,
} from "../../../imports/k8s";

export class NetworkBandwidthAnnotatorChart extends Chart {
  constructor(scope: Construct, id: string, props?: ChartProps) {
    super(scope, id, props);

    const ns = new KubeNamespace(this, "ns", {
      metadata: {
        name: "nba",
      },
    });

    const name = "network-bandwidth-annotator";
    const namespace = ns.name;
    const secretName = "tls-network-bandwidth-annotator";

    const selfsignedIssuer = new ClusterIssuer(this, "cluster-issuer", {
      metadata: {
        name: "selfsigned-issuer",
      },
      spec: {
        selfSigned: {},
      },
    });

    const cert = new Certificate(this, "cert", {
      metadata: {
        name,
        namespace,
      },
      spec: {
        secretName,
        duration: "2160h",
        renewBefore: "360h",
        subject: {
          organizations: ["Thomas Kosiewski"],
        },
        isCa: false,
        privateKey: {
          algorithm: CertificateSpecPrivateKeyAlgorithm.RSA,
          encoding: CertificateSpecPrivateKeyEncoding.PKCS1,
          size: 2048,
        },
        usages: [
          CertificateSpecUsages.SERVER_AUTH,
          CertificateSpecUsages.CLIENT_AUTH,
        ],
        dnsNames: [
          `${name}.${namespace}.svc`,
          `${name}.${namespace}.svc.cluster.local`,
        ],
        issuerRef: {
          name: selfsignedIssuer.name,
          kind: selfsignedIssuer.kind,
        },
      },
    });

    const labels = {
      app: name,
    };

    new KubeDeployment(this, "deployment", {
      metadata: {
        name,
        namespace,
      },
      spec: {
        replicas: 1,
        selector: {
          matchLabels: labels,
        },
        template: {
          metadata: {
            labels,
          },
          spec: {
            containers: [
              {
                name,
                image: "default-registry:61940/networkbandwidthannotator:0.1.0",
                command: ["./network-bandwidth-annotator", "-v"],
                ports: [{ containerPort: 8443, name: "https" }],
                env: [
                  {
                    name: "ADDR",
                    value: "0.0.0.0:8443",
                  },
                  {
                    name: "TLS_CERT_FILE",
                    value: "/certs/tls.crt",
                  },
                  {
                    name: "TLS_KEY_FILE",
                    value: "/certs/tls.key",
                  },
                ],
                volumeMounts: [
                  {
                    name: "tls-certs",
                    mountPath: "/certs",
                    readOnly: true,
                  },
                ],
              },
            ],
            volumes: [
              {
                name: "tls-certs",
                secret: {
                  secretName,
                },
              },
            ],
          },
        },
      },
    });

    new KubeService(this, "svc", {
      metadata: {
        name,
        namespace,
      },
      spec: {
        selector: labels,
        ports: [
          {
            port: 8443,
            name: "https",
          },
        ],
      },
    });

    new KubeMutatingWebhookConfiguration(this, "mwc", {
      metadata: {
        name,
        namespace,
        annotations: {
          "cert-manager.io/inject-ca-from": `${namespace}/${cert.name}`,
        },
      },
      webhooks: [
        {
          name: `${name}.${namespace}.svc`,
          clientConfig: {
            service: {
              name,
              namespace,
              path: "/mutate",
              port: 8443,
            },
          },
          namespaceSelector: {
            matchLabels: {
              "nba-enabled": "true",
            },
          },
          rules: [
            {
              apiGroups: [""],
              apiVersions: ["v1"],
              operations: ["CREATE", "UPDATE"],
              resources: ["pods"],
              scope: "Namespaced",
            },
          ],
          failurePolicy: "Ignore",
          admissionReviewVersions: ["v1", "v1beta1"],
          sideEffects: "None",
          timeoutSeconds: 5,
        },
      ],
    });
  }
}
