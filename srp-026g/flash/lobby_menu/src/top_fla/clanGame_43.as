package top_fla
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   
   public dynamic class clanGame_43 extends MovieClip
   {
      
      public var checkBoxClan:RadioSharedTop;
      
      public var checkBoxRankBattle:CheckBoxSkinnedIcon;
      
      public var sectionBack:MovieClip;
      
      public var checkBoxTournamentBattle:CheckBoxSkinnedIcon;
      
      public function clanGame_43()
      {
         super();
         this.__setProp_checkBoxRankBattle_clanGame_();
         this.__setProp_checkBoxTournamentBattle_clanGame_();
         this.__setProp_checkBoxClan_clanGame_();
      }
      
      internal function __setProp_checkBoxRankBattle_clanGame_() : *
      {
         try
         {
            this.checkBoxRankBattle["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkBoxRankBattle.autoSize = "none";
         this.checkBoxRankBattle.data = "";
         this.checkBoxRankBattle.enabled = false;
         this.checkBoxRankBattle.enableInitCallback = false;
         this.checkBoxRankBattle.focusable = true;
         this.checkBoxRankBattle.label = "";
         this.checkBoxRankBattle.selected = false;
         this.checkBoxRankBattle.visible = true;
         try
         {
            this.checkBoxRankBattle["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_checkBoxTournamentBattle_clanGame_() : *
      {
         try
         {
            this.checkBoxTournamentBattle["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkBoxTournamentBattle.autoSize = "none";
         this.checkBoxTournamentBattle.data = "";
         this.checkBoxTournamentBattle.enabled = false;
         this.checkBoxTournamentBattle.enableInitCallback = false;
         this.checkBoxTournamentBattle.focusable = true;
         this.checkBoxTournamentBattle.label = "";
         this.checkBoxTournamentBattle.selected = false;
         this.checkBoxTournamentBattle.visible = true;
         try
         {
            this.checkBoxTournamentBattle["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_checkBoxClan_clanGame_() : *
      {
         try
         {
            this.checkBoxClan["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkBoxClan.autoSize = "none";
         this.checkBoxClan.data = "";
         this.checkBoxClan.enabled = false;
         this.checkBoxClan.enableInitCallback = false;
         this.checkBoxClan.focusable = true;
         this.checkBoxClan.label = "";
         this.checkBoxClan.selected = false;
         this.checkBoxClan.visible = true;
         try
         {
            this.checkBoxClan["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

