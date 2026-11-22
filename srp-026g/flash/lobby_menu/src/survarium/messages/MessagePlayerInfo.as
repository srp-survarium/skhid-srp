package survarium.messages
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.Events;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IPlayerProfile;
   import survarium.player.ProfileSelector;
   import survarium.watch.WatchWindow;
   
   public class MessagePlayerInfo extends MessageInfoBase
   {
      
      public var btnAccept:Button;
      
      public var playerProfile:IPlayerProfile;
      
      public var profileSelector:ProfileSelector;
      
      public var textFieldLabel:TextField;
      
      public var textFieldNickname:TextField;

      // srp's Matches: the button left of Close, the player's matches in the Watch window (Replays);
      // none over the results screen: opening Watch changes the top menu's tab, which closes the
      // results (LobbyMenu.closeVS) and a drop card or reward still to take on them
      protected var _matchesButton:Button;
      
      public function MessagePlayerInfo()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         var profileSelectorRendrer:Class = getDefinitionByName("SrvProfileSelector") as Class;
         profileSelector = new profileSelectorRendrer();
         this.profileSelector.local = false;
         (this.playerProfile as MovieClip).addChild(this.profileSelector);
         this.playerProfile.registerInventory(_data.inventoryList);
         this.profileSelector.setupProfiles(_data.profiles);
         this.profileSelector.addEventListener(Events.PROFILE_SELECTED,this.onProfileSelected);
         this.profileSelector.selectProfile(0);
         this.profileSelector.x = 9;
         this.profileSelector.y = 42;
         this.btnAccept.label = LocalizationData.getName("close");
         this.textFieldLabel.text = LocalizationData.getName("player_info_label");
         this.btnAccept.addEventListener(ButtonEvent.CLICK,onMessageAccept,false,0,true);
         this.textFieldNickname.text = _data.nickname;
         if(LobbyMenu.id != null && LobbyMenu.id.resultsUp)
         {
            return;
         }
         this._matchesButton = new (Object(this.btnAccept).constructor as Class)() as Button;
         this._matchesButton.name = "srpPlayerMatches";
         this._matchesButton.label = LocalizationData.getName("st_srp_player_matches");
         this._matchesButton.y = this.btnAccept.y;
         this._matchesButton.width = Math.max(this.btnAccept.width,int(this._matchesButton.label.length * 8 + 30));
         this._matchesButton.x = this.btnAccept.x - 8 - this._matchesButton.width;
         this._matchesButton.addEventListener(ButtonEvent.CLICK,this.onMatches,false,0,true);
         addChild(this._matchesButton);
      }

      // The pop-up closes, and the Watch window opens on Replays, this player's name searched.
      protected function onMatches(e:ButtonEvent) : void
      {
         var nickname:String = String(_data.nickname);
         onMessageAccept();
         WatchWindow.openPlayer(nickname);
      }
      
      private function onProfileSelected(e:Event) : void
      {
         trace("onProfileSelected");
         this.playerProfile.fillProfileItems(this.profileSelector.getSelectedProfileData());
      }
   }
}

