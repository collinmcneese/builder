// Copyright (c) 2016-2017 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import {
  AfterViewChecked, Component, ElementRef, EventEmitter, Input, OnChanges, Output,
  SimpleChanges, ViewChild
} from '@angular/core';
import { FormBuilder, FormGroup } from '@angular/forms';
import { Router } from '@angular/router';
import { MatDialog } from '@angular/material';
import { Record } from 'immutable';
import { DisconnectConfirmDialog } from './dialog/disconnect-confirm/disconnect-confirm.dialog';
import { DockerExportSettingsComponent } from '../../shared/docker-export-settings/docker-export-settings.component';
import { BuilderApiClient } from '../../client/builder-api';
import { AppStore } from '../../app.store';
import {
  addProject, updateProject, setProjectIntegrationSettings, deleteProject,
  fetchGitHubInstallations, fetchGitHubRepositories, fetchProject, setProjectVisibility,
  deleteProjectIntegration
} from '../../actions/index';
import config from '../../config';
import { targetFrom } from '../../util';

@Component({
  selector: 'hab-project-settings',
  template: require('./project-settings.component.html')
})
export class ProjectSettingsComponent implements OnChanges, AfterViewChecked {
  connecting: boolean = false;
  doesFileExist: Function;
  form: FormGroup;
  activeInstallation: any;
  activeRepo: any;
  selectedInstallation: any;
  selectedRepo: string;
  selectedPath: string;
  planPathValidators: any[];

  @Input() integrations;
  @Input() name: string;
  @Input() origin: string;
  @Input() project: any;
  @Input() projects = [];
  @Input() target: string;

  @Output() saved: EventEmitter<any> = new EventEmitter<any>();
  @Output() toggled: EventEmitter<any> = new EventEmitter<any>();

  @ViewChild('docker')
  docker: DockerExportSettingsComponent;

  private api: BuilderApiClient;
  private _visibility: string;
  private _autoBuild;

  private _doAfterViewChecked: Function[] = [];

  constructor(
    private formBuilder: FormBuilder,
    private router: Router,
    private store: AppStore,
    private disconnectDialog: MatDialog,
    private elementRef: ElementRef
  ) {
    this.api = new BuilderApiClient(this.token);
    this.selectedPath = this.defaultPath;

    this.doesFileExist = (path) => {
      if (!this.selectedInstallation) {
        return new Promise(resolve => resolve(null));
      }

      return this.api.findFileInRepo(
        this.selectedInstallation.get('installation_id'),
        this.selectedInstallation.get('org'),
        this.activeRepo.get('id'),
        this.planField.value
      );
    };

    this.planPathValidators = [
      {
        error: 'invalidFilename',
        message: this.invalidFilenameMessage.bind(this),
        validator: this.filenameValidator.bind(this)
      }
    ];
  }

  invalidFilenameMessage() {
    const ext = this.isWindowsTarget ? 'ps1' : 'sh';
    const msg = `A ${this.planTargetName} plan file name must end with .${ext}`;
    return msg;
  }

  filenameValidator(control) {
    const value = control.value;

    if (this.isWindowsTarget) {
      return value.slice(-4) === '.ps1' ? null : { invalidFilename: true };
    } else {
      return value.slice(-3) === '.sh' ? null : { invalidFilename: true };
    }
  }

  ngAfterViewChecked() {
    const f = this._doAfterViewChecked.shift();

    if (f) {
      f();
    }
  }

  ngOnChanges(changes: SimpleChanges) {
    const p = changes['project'];
    const target = changes['target'];

    if (p && p.currentValue) {
      this.selectedRepo = p.currentValue.vcs_data;
      this.selectedPath = p.currentValue.plan_path;
      this.visibility = p.currentValue.visibility || this.visibility;
    }

    if (target && target.currentValue) {
      if (this.projects.filter(p => p.target === target.currentValue).length) {
        console.log('changes target e', target.currentValue);
        this.editConnection(target.currentValue);
      } else {
        console.log('changes target c', target.currentValue);
        this.connect(target.currentValue);
      }
    }
  }

  get autoBuild() {

    if (typeof this._autoBuild === 'undefined') {
      this._autoBuild = !!this.store.getState().projects.current.auto_build;
    }

    return this._autoBuild;
  }

  set autoBuild(v: boolean) {
    this._autoBuild = v;
  }

  get config() {
    return config;
  }

  get isUpdating() {
    return this.target && this.projects.filter(p => p.target === this.target).length;
  }

  get connectButtonLabel() {
    return this.isUpdating ? 'Update' : 'Save';
  }

  get dockerEnabled() {
    return this.dockerSettings && this.dockerSettings.size > 0;
  }

  get dockerSettings() {
    return this.store.getState().projects.current.settings;
  }

  get files() {
    return this.store.getState().gitHub.files;
  }

  get defaultPath() {
    const ext = this.isWindowsTarget ? 'ps1' : 'sh';
    return `habitat/plan.${ext}`;
  }

  get gitHubAppNote() {

    let note = `In order to connect a plan file in your repo,
      you must first install the Builder GitHub app
      and allow access to that repository.`;

    if (this.gitHubAppInstalled) {
      note = `If you don't see one or more of your organizations
        or repositories listed below, you may need to adjust the
        settings of the Builder GitHub app.`;
    }

    return note;
  }

  get gitHubAppLabel() {
    let label = 'Install';

    if (this.gitHubAppInstalled) {
      label = 'Open';
    }

    return label;
  }

  get gitHubAppInstalled() {
    return !this.loadingInstallations && this.installations.size > 0;
  }

  get hasPrivateKey() {
    const currentOrigin = this.store.getState().origins.current;
    return currentOrigin.name === this.origin && !!currentOrigin.private_key_name;
  }

  get installations() {
    return this.store.getState().gitHub.installations;
  }

  get loadingInstallations() {
    return this.store.getState().gitHub.ui.installations.loading;
  }

  get loadingRepositories() {
    return this.store.getState().gitHub.ui.repositories.loading;
  }

  get orgs() {
    return this.store.getState().gitHub.orgs;
  }

  get planField() {
    return this.form.controls['plan_path'];
  }

  get planTemplate() {
    return {
      'origin': this.origin,
      'plan_path': this.planField.value,
      'installation_id': this.selectedInstallation.get('installation_id'),
      'repo_id': this.activeRepo.get('id'),
      'auto_build': this.autoBuild,
      'target': this.target
    };
  }

  get planTargetName() {
    const target = targetFrom('id', this.target);
    return target ? target.name : null;
  }

  get isWindowsTarget() {
    return this.target === 'x86_64-windows';
  }

  get repoField() {
    return this.form.controls['repo_path'];
  }

  get repositories() {
    return this.store.getState().gitHub.repositories;
  }

  get repoUrl() {
    if (this.selectedInstallation) {
      return `https://github.com/${this.selectedInstallation.get('full_name')}`;
    }
  }

  get token() {
    return this.store.getState().session.token;
  }

  get username() {
    return this.store.getState().users.current.profile.name;
  }

  get repoSelected() {
    return this.activeInstallation && this.activeRepo;
  }

  get validProject() {
    const planPathValid = this.planField ? this.planField.valid : false;
    const dockerValid = (this.docker && this.docker.settings.enabled) ? this.docker.settings.valid : true;
    return this.selectedInstallation && dockerValid && planPathValid;
  }

  get visibility() {
    return this._visibility || this.store.getState().origins.current.default_package_visibility || 'public';
  }

  set visibility(v: string) {
    this._visibility = v;
  }

  autoBuildToggled(v: boolean) {
    this.autoBuild = v;
  }

  openConnect(target) {
    this.router.navigate(['/pkgs', this.origin, this.name, 'settings', target]);
  }

  connect(target) {
    this.deselect();
    this.store.dispatch(fetchGitHubInstallations(this.username));
    this.connecting = true;
    this.toggled.emit(this.connecting);
  }

  disconnect(project) {
    const ref = this.disconnectDialog.open(DisconnectConfirmDialog, {
      width: '460px'
    });

    ref.afterClosed().subscribe((confirmed) => {
      if (confirmed) {
        this.store.dispatch(
          deleteProject(project.origin, project.package_name, project.target, this.token)
        );
      }
    });
  }

  isWindows(path) {
    return !!path.match(/\.ps1$/);
  }

  clearConnection() {
    this.clearSelection();
    this.router.navigate(['/pkgs', this.origin, this.name, 'settings']);
  }

  clearSelection() {
    this.connecting = false;
    this.deselect();
    this.toggled.emit(this.connecting);
    window.scroll(0, 0);
  }

  deselect() {
    this.form = this.formBuilder.group({});
    this.selectedRepo = null;
    this.activeInstallation = null;
    this.activeRepo = null;
    this.selectedInstallation = null;
    this.selectedPath = this.defaultPath;
  }

  openConnectEdit(project) {
    const target = targetFrom('id', project.target);
    this.openConnect(target.param);
  }

  editConnection(target) {
    const project = this.projects.filter(p => p.target === target)[0];
    this.connect(project.target);

    this.selectedPath = project.plan_path;
    this.selectedRepo = this.parseGitHubUrl(project.vcs_data);
    const [org, name] = this.selectedRepo.split('/');

    // This looks a bit weird, but it allows us to scroll the selected
    // org and repo into view. What we're doing is asking to be notified
    // when orgs and repos have been loaded, and delaying execution of the
    // functions that do that scrolling until Angular's gone through a
    // rendering cycle for each of those lists individually.
    const unsubInstalls = this.store.subscribe(state => {
      const installs = state.gitHub.installations;

      if (installs.size > 0) {
        unsubInstalls();

        installs.forEach(i => {
          if (i.get('account').get('login') === org) {
            this.pickInstallation(i);

            this.doAfterViewChecked(() => {
              this.elementRef.nativeElement.querySelector('.installations .active').scrollIntoView();
            });
          }
        });
      }
    });

    const unsubRepos = this.store.subscribe(state => {
      const repos = state.gitHub.repositories;

      if (repos.size > 0) {
        unsubRepos();

        repos.forEach(repo => {
          if (repo.get('name') === name) {
            this.pickRepo(repo);

            this.doAfterViewChecked(() => {
              this.elementRef.nativeElement.querySelector('.repositories .active').scrollIntoView();
            });
          }
        });
      }
    });
  }

  next() {
    this.selectRepository(this.activeRepo);
  }

  pickInstallation(install) {
    this.activeInstallation = install;
    this.activeRepo = null;
    this.store.dispatch(fetchGitHubRepositories(install.get('id')));
  }

  pickRepo(repo) {
    this.activeRepo = repo;
    this.selectRepository(this.activeRepo);
  }

  saveConnection() {
    if (this.isUpdating) {
      this.store.dispatch(updateProject(this.project.name, this.planTemplate, this.token, (result) => {
        this.handleSaved(result.success, this.project.origin, this.project.package_name);
      }));
    }
    else {
      this.store.dispatch(addProject(this.planTemplate, this.token, (result) => {
        this.handleSaved(result.success, result.response.origin, result.response.package_name);
      }));
    }
  }

  selectRepository(repo) {
    this.selectedInstallation = Record({
      repo_id: repo.get('id'),
      app_id: this.config.github_app_id,
      installation_id: this.activeInstallation.get('id'),
      full_name: repo.get('full_name'),
      org: repo.get('owner').get('login'),
      name: repo.get('name'),
      url: repo.get('url')
    })();

    if (this.planField) {
      this.planField.dirty ? this.planField.updateValueAndValidity() : this.planField.markAsDirty();
    }
  }

  settingChanged(setting) {
    this.visibility = setting;
    this.saveVisibility(this.origin, this.name);
  }

  private doAfterViewChecked(f) {
    this._doAfterViewChecked.push(f);
  }

  private handleSaved(successful, origin, name) {
    if (successful) {
      this.saveIntegration(origin, name);
      this.store.dispatch(fetchProject(origin, name, this.token, false));
      this.saved.emit({ origin: origin, name: name });
      this.clearConnection();
    }
  }

  private parseGitHubUrl(url) {
    return (url.match(/github.com\/(.+)\.git$/) || [''])[1] || '';
  }

  private saveIntegration(origin, name) {
    const settings = this.docker.settings;
    if (settings.enabled) {
      this.store.dispatch(
        setProjectIntegrationSettings(
          origin, name, settings.name, settings.settings, this.token
        )
      );
    }
    else {
      this.store.getState().projects.current.settings.map((v, k) => {
        this.store.dispatch(deleteProjectIntegration(this.origin, this.name, k, this.token));
      });
    }
  }

  private saveVisibility(origin, name) {
    this.store.dispatch(setProjectVisibility(origin, name, this.visibility, this.token));
  }
}
