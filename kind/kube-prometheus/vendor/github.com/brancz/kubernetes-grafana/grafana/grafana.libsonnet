local defaults = {
  local defaults = self,
  namespace: 'default',
  version: '7.5.10',
  image: 'docker.io/grafana/grafana:' + defaults.version,
  commonLabels:: {
    'app.kubernetes.io/name': 'grafana',
    'app.kubernetes.io/version': defaults.version,
    'app.kubernetes.io/component': 'grafana',
  },
  selectorLabels:: {
    [labelName]: defaults.commonLabels[labelName]
    for labelName in std.objectFields(defaults.commonLabels)
    if !std.setMember(labelName, ['app.kubernetes.io/version'])
  },
  replicas: 1,
  port: 3000,
  resources: {
    requests: { cpu: '100m', memory: '100Mi' },
    limits: { cpu: '200m', memory: '200Mi' },
  },

  dashboards: {},
  rawDashboards: {},
  folderDashboards: {},
  folderUidGenerator(folder): '',
  datasources: [{
    name: 'prometheus',
    type: 'prometheus',
    access: 'proxy',
    orgId: 1,
    url: 'http://prometheus-k8s.' + defaults.namespace + '.svc:9090',
    version: 1,
    editable: false,
  }],
  // Forces pod restarts when dashboards are changed
  dashboardsChecksum: false,
  config: {
    sections: {
      date_formats: { default_timezone: 'UTC' },
    },
  },
  ldap: null,
  plugins: [],
  env: [],
  containers: [],
};

function(params) {
  local g = self,
  _config:: defaults + params,
  _metadata:: {
    name: 'grafana',
    namespace: g._config.namespace,
    labels: g._config.commonLabels,
  },

  serviceAccount: {
    apiVersion: 'v1',
    kind: 'ServiceAccount',
    metadata: g._metadata,
    automountServiceAccountToken: false,
  },

  service: {
    apiVersion: 'v1',
    kind: 'Service',
    metadata: g._metadata,
    spec: {
      selector: g.deployment.spec.selector.matchLabels,
      ports: [
        { name: 'http', targetPort: 'http', port: 3000 },
      ],
    },
  },

  config: {
    apiVersion: 'v1',
    kind: 'Secret',
    metadata: g._metadata {
      name: 'grafana-config',
    },
    type: 'Opaque',
    stringData: {
      'grafana.ini': std.manifestIni(g._config.config),
    } + if g._config.ldap != null then { 'ldap.toml': g._config.ldap } else {},
  },

  dashboardDefinitions: {
    apiVersion: 'v1',
    kind: 'ConfigMapList',
    items: [
      {
        local dashboardName = 'grafana-dashboard-' + std.strReplace(name, '.json', ''),
        apiVersion: 'v1',
        kind: 'ConfigMap',
        metadata: g._metadata {
          name: dashboardName,
        },
        data: { [name]: std.manifestJsonEx(g._config.dashboards[name], '    ') },
      }
      for name in std.objectFields(g._config.dashboards)
    ] + [
      {
        local dashboardName = 'grafana-dashboard-' + std.strReplace(name, '.json', ''),
        apiVersion: 'v1',
        kind: 'ConfigMap',
        metadata: g._metadata {
          name: dashboardName,
        },
        data: { [name]: std.manifestJsonEx(g._config.folderDashboards[folder][name], '    ') },
      }
      for folder in std.objectFields(g._config.folderDashboards)
      for name in std.objectFields(g._config.folderDashboards[folder])
    ] + (
      if std.length(g._config.rawDashboards) > 0 then
        [

          {
            local dashboardName = 'grafana-dashboard-' + std.strReplace(name, '.json', ''),
            apiVersion: 'v1',
            kind: 'ConfigMap',
            metadata: g._metadata {
              name: dashboardName,
            },
            data: { [name]: g._config.rawDashboards[name] },
          }
          for name in std.objectFields(g._config.rawDashboards)
        ]
      else
        []
    ),
  },

  dashboardSources:
    local dashboardSources = {
      apiVersion: 1,
      providers:
        (
          if std.length(g._config.dashboards) +
             std.length(g._config.rawDashboards) > 0 then [
            {
              name: '0',
              orgId: 1,
              folder: 'Default',
              folderUid: g._config.folderUidGenerator('Default'),
              type: 'file',
              options: {
                path: '/grafana-dashboard-definitions/0',
              },
            },
          ] else []
        ) +
        [
          {
            name: folder,
            orgId: 1,
            folder: folder,
            folderUid: g._config.folderUidGenerator(folder),
            type: 'file',
            options: {
              path: '/grafana-dashboard-definitions/' + folder,
            },
          }
          for folder in std.objectFields(g._config.folderDashboards)
        ],
    };

    {
      kind: 'ConfigMap',
      apiVersion: 'v1',
      metadata: g._metadata {
        name: 'grafana-dashboards',
      },
      data: { 'dashboards.yaml': std.manifestJsonEx(dashboardSources, '    ') },
    },

  dashboardDatasources: {
    apiVersion: 'v1',
    kind: 'Secret',
    metadata: g._metadata {
      name: 'grafana-datasources',
    },
    type: 'Opaque',
    stringData: {
      'datasources.yaml': std.manifestJsonEx(
        {
          apiVersion: 1,
          datasources: g._config.datasources,
        }, '    '
      ),
    },
  },

  deployment:
    local configVolume = {
      name: 'grafana-config',
      secret: { secretName: g.config.metadata.name },
    };
    local configVolumeMount = {
      name: configVolume.name,
      mountPath: '/etc/grafana',
      readOnly: false,
    };

    local storageVolume = {
      name: 'grafana-storage',
      emptyDir: {},
    };
    local storageVolumeMount = {
      name: storageVolume.name,
      mountPath: '/var/lib/grafana',
      readOnly: false,
    };

    local datasourcesVolume = {
      name: 'grafana-datasources',
      secret: { secretName: g.dashboardDatasources.metadata.name },
    };
    local datasourcesVolumeMount = {
      name: datasourcesVolume.name,
      mountPath: '/etc/grafana/provisioning/datasources',
      readOnly: false,
    };

    local dashboardsVolume = {
      name: 'grafana-dashboards',
      configMap: { name: g.dashboardSources.metadata.name },
    };
    local dashboardsVolumeMount = {
      name: dashboardsVolume.name,
      mountPath: '/etc/grafana/provisioning/dashboards',
      readOnly: false,
    };
    // A volume on /tmp is needed to let us use 'readOnlyRootFilesystem: true'
    local pluginTmpVolume = {
      name: 'tmp-plugins',
      emptyDir: {
        medium: 'Memory',
      },
    };
    local pluginTmpVolumeMount = {
      mountPath: '/tmp',
      name: 'tmp-plugins',
      readOnly: false,
    };

    local volumeMounts =
      [
        storageVolumeMount,
        datasourcesVolumeMount,
        dashboardsVolumeMount,
        pluginTmpVolumeMount,
      ] +
      [
        {
          local dashboardName = std.strReplace(name, '.json', ''),
          name: 'grafana-dashboard-' + dashboardName,
          mountPath: '/grafana-dashboard-definitions/0/' + dashboardName,
          readOnly: false,
        }
        for name in std.objectFields(g._config.dashboards + g._config.rawDashboards)
      ] +
      [
        {
          local dashboardName = std.strReplace(name, '.json', ''),
          name: 'grafana-dashboard-' + dashboardName,
          mountPath: '/grafana-dashboard-definitions/' + folder + '/' + dashboardName,
          readOnly: false,
        }
        for folder in std.objectFields(g._config.folderDashboards)
        for name in std.objectFields(g._config.folderDashboards[folder])
      ] + (
        if std.length(g._config.config) > 0 then [configVolumeMount] else []
      );

    local volumes =
      [
        storageVolume,
        datasourcesVolume,
        dashboardsVolume,
        pluginTmpVolume,
      ] +
      [
        {
          local dashboardName = 'grafana-dashboard-' + std.strReplace(name, '.json', ''),
          name: dashboardName,
          configMap: { name: dashboardName },
        }
        for name in std.objectFields(g._config.dashboards)
      ] +
      [
        {
          local dashboardName = 'grafana-dashboard-' + std.strReplace(name, '.json', ''),
          name: dashboardName,
          configMap: { name: dashboardName },
        }
        for folder in std.objectFields(g._config.folderDashboards)
        for name in std.objectFields(g._config.folderDashboards[folder])
      ] +
      [
        {
          local dashboardName = 'grafana-dashboard-' + std.strReplace(name, '.json', ''),
          name: dashboardName,
          configMap: { name: dashboardName },
        }
        for name in std.objectFields(g._config.rawDashboards)
      ] +
      if std.length(g._config.config) > 0 then [configVolume] else [];

    local plugins = (
      if std.length(g._config.plugins) == 0 then
        []
      else
        [{ name: 'GF_INSTALL_PLUGINS', value: std.join(',', g._config.plugins) }]
    );

    local grafanaContainer = {
      name: 'grafana',
      image: g._config.image,
      env: g._config.env + plugins,
      volumeMounts: volumeMounts,
      ports: [{
        name: 'http',
        containerPort: g._config.port,
      }],
      readinessProbe: {
        httpGet: {
          path: '/api/health',
          port: grafanaContainer.ports[0].name,
        },
      },
      resources: g._config.resources,
      securityContext: {
        capabilities: { drop: ['ALL'] },
        allowPrivilegeEscalation: false,
        readOnlyRootFilesystem: true,
        seccompProfile: { type: 'RuntimeDefault' },
      },
    };

    {
      apiVersion: 'apps/v1',
      kind: 'Deployment',
      metadata: g._metadata,
      spec: {
        replicas: g._config.replicas,
        selector: {
          matchLabels: g._config.selectorLabels,
        },
        template: {
          metadata: {
            labels: g._config.commonLabels,
            annotations: {
              [if std.length(g._config.config) > 0 then 'checksum/grafana-config']: std.md5(std.toString(g.config)),
              'checksum/grafana-datasources': std.md5(std.toString(g.dashboardDatasources)),
              [if g._config.dashboardsChecksum then 'checksum/grafana-dashboards']: std.md5(std.toString(g.dashboardDefinitions)),
              'checksum/grafana-dashboardproviders': std.md5(std.toString(g.dashboardSources)),
            },
          },
          spec: {
            containers: [grafanaContainer] + g._config.containers,
            volumes: volumes,
            serviceAccountName: g.serviceAccount.metadata.name,
            nodeSelector: {
              'kubernetes.io/os': 'linux',
            },
            securityContext: {
              fsGroup: 65534,
              runAsNonRoot: true,
              runAsUser: 65534,
            },
          },
        },
      },
    },
}
