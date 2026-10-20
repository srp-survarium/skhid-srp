package main_menu_fla
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
   
   public dynamic class avatarList_19 extends MovieClip
   {
      
      public var btn_cancel:MessageButton;
      
      public var btn_set:MessageButton;
      
      public var list:SrvInventoryListcopy;
      
      public function avatarList_19()
      {
         super();
         this.__setProp_list_avatarList_Layer1_0();
         this.__setProp_btn_set_avatarList_Layer1_0();
         this.__setProp_btn_cancel_avatarList_Layer1_0();
      }
      
      internal function __setProp_list_avatarList_Layer1_0() : *
      {
         try
         {
            this.list["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.list.columnWidth = 100;
         this.list.direction = "vertical";
         this.list.draggable = false;
         this.list.enabled = true;
         this.list.enableInitCallback = false;
         this.list.externalColumnCount = 0;
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
         this.list.scrollBar = "SrvScrollBar";
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
      
      internal function __setProp_btn_set_avatarList_Layer1_0() : *
      {
         try
         {
            this.btn_set["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.btn_set.autoRepeat = false;
         this.btn_set.autoSize = "none";
         this.btn_set.data = "";
         this.btn_set.enabled = true;
         this.btn_set.enableInitCallback = false;
         this.btn_set.focusable = true;
         this.btn_set.label = "OK";
         this.btn_set.selected = false;
         this.btn_set.toggle = false;
         this.btn_set.visible = true;
         try
         {
            this.btn_set["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_cancel_avatarList_Layer1_0() : *
      {
         try
         {
            this.btn_cancel["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         this.btn_cancel.autoRepeat = false;
         this.btn_cancel.autoSize = "none";
         this.btn_cancel.data = "";
         this.btn_cancel.enabled = true;
         this.btn_cancel.enableInitCallback = false;
         this.btn_cancel.focusable = true;
         this.btn_cancel.label = "Отмена";
         this.btn_cancel.selected = false;
         this.btn_cancel.toggle = false;
         this.btn_cancel.visible = true;
         try
         {
            this.btn_cancel["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

