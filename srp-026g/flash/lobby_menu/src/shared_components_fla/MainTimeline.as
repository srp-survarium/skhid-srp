package shared_components_fla
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
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var btnNoscale:SrvButtonScale;
      
      public var btnFilter:FilterOnlineButtonDef;
      
      public var scrollBar:SrvScrollBarSkinned;
      
      public var btnClose:CloseButtonDef;
      
      public var btnNormal:SrvButtonSkinned;
      
      public var text_inpput:SrvTextInputShared;
      
      public var progressIndicator:LoadingIndicatorShared;
      
      public var btnIgnore:FilterIgnoreButtonDef;
      
      public var list:SrvScrollingList;
      
      public var btnHide:HideButtonDef;
      
      public var playersList:SrvScrollingListSD;
      
      public var questIcon:QuestIcon;
      
      public var btnNormalRed:SrvButtonSkinnedRed;
      
      public var profileTab:TabBaseAssets;
      
      public var checkbox_radio_short:RadioSharedShort;
      
      public var checkbox_normal:CheckBoxShared;
      
      public var checkBoxRegularCost:SrvCheckBoxSkinnedValue;
      
      public var srvProfileTab:SrvButtonTab;
      
      public var objectIcon:ObjectIcon;
      
      public var checkbox_radio:RadioShared;
      
      public function MainTimeline()
      {
         super();
         this.__setProp_checkbox_normal_Scene1_();
         this.__setProp_checkbox_radio_Scene1_();
         this.__setProp_list_Scene1_();
         this.__setProp_playersList_Scene1_();
         this.__setProp_text_inpput_Scene1_();
         this.__setProp_checkbox_radio_short_Scene1_();
         this.__setProp_btnNormal_Scene1_buttons_0();
         this.__setProp_btnNormalRed_Scene1_buttons_0();
         this.__setProp_btnNoscale_Scene1_buttons_0();
      }
      
      internal function __setProp_checkbox_normal_Scene1_() : *
      {
         try
         {
            this.checkbox_normal["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkbox_normal.autoSize = "none";
         this.checkbox_normal.data = "";
         this.checkbox_normal.enabled = false;
         this.checkbox_normal.enableInitCallback = false;
         this.checkbox_normal.focusable = true;
         this.checkbox_normal.label = "";
         this.checkbox_normal.selected = false;
         this.checkbox_normal.visible = true;
         try
         {
            this.checkbox_normal["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_checkbox_radio_Scene1_() : *
      {
         try
         {
            this.checkbox_radio["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkbox_radio.autoSize = "none";
         this.checkbox_radio.data = "";
         this.checkbox_radio.enabled = false;
         this.checkbox_radio.enableInitCallback = false;
         this.checkbox_radio.focusable = true;
         this.checkbox_radio.label = "";
         this.checkbox_radio.selected = false;
         this.checkbox_radio.visible = true;
         try
         {
            this.checkbox_radio["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_list_Scene1_() : *
      {
         try
         {
            this.list["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.list.enabled = true;
         this.list.enableInitCallback = false;
         this.list.focusable = true;
         this.list.itemRendererName = "";
         this.list.itemRendererInstanceName = "";
         this.list.margin = 0;
         this.list.inspectablePadding = {
            "top":0,
            "right":0,
            "bottom":0,
            "left":0
         };
         this.list.rowHeight = 100;
         this.list.scrollBar = "";
         this.list.visible = true;
         this.list.wrapping = "normal";
         try
         {
            this.list["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_playersList_Scene1_() : *
      {
         try
         {
            this.playersList["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.playersList.draggable = false;
         this.playersList.enabled = true;
         this.playersList.enableInitCallback = false;
         this.playersList.focusable = true;
         this.playersList.itemRendererName = "";
         this.playersList.itemRendererInstanceName = "";
         this.playersList.margin = 2;
         this.playersList.inspectablePadding = {
            "top":4,
            "right":4,
            "bottom":4,
            "left":4
         };
         this.playersList.rowHeight = 35;
         this.playersList.scrollBar = "";
         this.playersList.scrollYPadding = 0;
         this.playersList.visible = true;
         this.playersList.wrapping = "normal";
         try
         {
            this.playersList["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_text_inpput_Scene1_() : *
      {
         try
         {
            this.text_inpput["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.text_inpput.actAsButton = true;
         this.text_inpput.defaultText = "";
         this.text_inpput.displayAsPassword = false;
         this.text_inpput.editable = true;
         this.text_inpput.enabled = true;
         this.text_inpput.enableInitCallback = false;
         this.text_inpput.focusable = true;
         this.text_inpput.maxChars = 120;
         this.text_inpput.text = "";
         this.text_inpput.visible = true;
         try
         {
            this.text_inpput["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_checkbox_radio_short_Scene1_() : *
      {
         try
         {
            this.checkbox_radio_short["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.checkbox_radio_short.autoSize = "none";
         this.checkbox_radio_short.data = "";
         this.checkbox_radio_short.enabled = false;
         this.checkbox_radio_short.enableInitCallback = false;
         this.checkbox_radio_short.focusable = true;
         this.checkbox_radio_short.label = "";
         this.checkbox_radio_short.selected = false;
         this.checkbox_radio_short.visible = true;
         try
         {
            this.checkbox_radio_short["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnNormal_Scene1_buttons_0() : *
      {
         try
         {
            this.btnNormal["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.btnNormal.autoRepeat = false;
         this.btnNormal.autoSize = "none";
         this.btnNormal.data = "";
         this.btnNormal.enabled = true;
         this.btnNormal.enableInitCallback = false;
         this.btnNormal.focusable = true;
         this.btnNormal.label = "label";
         this.btnNormal.selected = false;
         this.btnNormal.tooltipRendererName = "";
         this.btnNormal.toggle = false;
         this.btnNormal.visible = true;
         try
         {
            this.btnNormal["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnNormalRed_Scene1_buttons_0() : *
      {
         try
         {
            this.btnNormalRed["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.btnNormalRed.autoRepeat = false;
         this.btnNormalRed.autoSize = "none";
         this.btnNormalRed.data = "";
         this.btnNormalRed.enabled = true;
         this.btnNormalRed.enableInitCallback = false;
         this.btnNormalRed.focusable = true;
         this.btnNormalRed.label = "red_label";
         this.btnNormalRed.selected = false;
         this.btnNormalRed.tooltipRendererName = "";
         this.btnNormalRed.toggle = false;
         this.btnNormalRed.visible = true;
         try
         {
            this.btnNormalRed["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btnNoscale_Scene1_buttons_0() : *
      {
         try
         {
            this.btnNoscale["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.btnNoscale.autoRepeat = false;
         this.btnNoscale.autoSize = "none";
         this.btnNoscale.data = "";
         this.btnNoscale.enabled = true;
         this.btnNoscale.enableInitCallback = false;
         this.btnNoscale.focusable = true;
         this.btnNoscale.label = "label";
         this.btnNoscale.selected = false;
         this.btnNoscale.tooltipRendererName = "";
         this.btnNoscale.toggle = false;
         this.btnNoscale.visible = true;
         try
         {
            this.btnNoscale["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

