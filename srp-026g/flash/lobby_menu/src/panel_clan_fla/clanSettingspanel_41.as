package panel_clan_fla
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
   
   public dynamic class clanSettingspanel_41 extends MovieClip
   {
      
      public var rankName:SrvTextInputSharedCl;
      
      public var checkBoxKickRight:CheckBoxSharedCl;
      
      public var checkBoxBattleCommanderRight:CheckBoxSharedCl;
      
      public var checkBoxChangeRankRight:CheckBoxSharedCl;
      
      public var btnAddRank:SrvButtonSkinnedCl;
      
      public var btnDeleteRank:SrvButtonSkinnedCl;
      
      public var checkBoxDailyMessageRight:CheckBoxSharedCl;
      
      public var checkBoxInviteRight:CheckBoxSharedCl;
      
      public var ranksList:DefaultScrollingListPLayerList;
      
      public function clanSettingspanel_41()
      {
         super();
         this.__setProp_rankName_clanSettingspanel_();
         this.__setProp_ranksList_clanSettingspanel_();
      }
      
      internal function __setProp_rankName_clanSettingspanel_() : *
      {
         try
         {
            this.rankName["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.rankName.actAsButton = false;
         this.rankName.defaultText = "";
         this.rankName.displayAsPassword = false;
         this.rankName.editable = true;
         this.rankName.enabled = true;
         this.rankName.enableInitCallback = false;
         this.rankName.focusable = true;
         this.rankName.maxChars = 15;
         this.rankName.text = "";
         this.rankName.visible = true;
         try
         {
            this.rankName["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_ranksList_clanSettingspanel_() : *
      {
         try
         {
            this.ranksList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.ranksList.enabled = true;
         this.ranksList.enableInitCallback = false;
         this.ranksList.focusable = true;
         this.ranksList.itemRendererName = "BtnClanRank";
         this.ranksList.itemRendererInstanceName = "";
         this.ranksList.margin = 0;
         this.ranksList.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         this.ranksList.rowHeight = 38;
         this.ranksList.scrollBar = "";
         this.ranksList.visible = true;
         this.ranksList.wrapping = "normal";
         try
         {
            this.ranksList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

