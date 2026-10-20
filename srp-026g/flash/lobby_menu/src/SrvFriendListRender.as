package
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
   import survarium.friends.FriendListRender;
   
   public dynamic class SrvFriendListRender extends FriendListRender
   {
      
      public function SrvFriendListRender()
      {
         super();
         addFrameScript(0,this.frame1,9,this.frame10,19,this.frame20,29,this.frame30,39,this.frame40,40,this.frame41,49,this.frame50,59,this.frame60,69,this.frame70,79,this.frame80);
         this.__setProp_btn_cancel_SrvFriendListRender_buttons_0();
         this.__setProp_btn_acccept_SrvFriendListRender_buttons_0();
      }
      
      internal function __setProp_btn_cancel_SrvFriendListRender_buttons_0() : *
      {
         try
         {
            btn_cancel["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btn_cancel.autoRepeat = false;
         btn_cancel.autoSize = "none";
         btn_cancel.data = "";
         btn_cancel.enabled = true;
         btn_cancel.enableInitCallback = false;
         btn_cancel.focusable = true;
         btn_cancel.label = "";
         btn_cancel.selected = false;
         btn_cancel.toggle = true;
         btn_cancel.visible = true;
         try
         {
            btn_cancel["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_btn_acccept_SrvFriendListRender_buttons_0() : *
      {
         try
         {
            btn_acccept["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         btn_acccept.autoRepeat = false;
         btn_acccept.autoSize = "none";
         btn_acccept.data = "";
         btn_acccept.enabled = true;
         btn_acccept.enableInitCallback = false;
         btn_acccept.focusable = true;
         btn_acccept.label = "";
         btn_acccept.selected = false;
         btn_acccept.toggle = true;
         btn_acccept.visible = true;
         try
         {
            btn_acccept["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function frame1() : *
      {
         preventAutosizing = true;
      }
      
      internal function frame10() : *
      {
         stop();
      }
      
      internal function frame20() : *
      {
         stop();
      }
      
      internal function frame30() : *
      {
         stop();
      }
      
      internal function frame40() : *
      {
         stop();
      }
      
      internal function frame41() : *
      {
      }
      
      internal function frame50() : *
      {
         stop();
      }
      
      internal function frame60() : *
      {
         stop();
      }
      
      internal function frame70() : *
      {
         stop();
      }
      
      internal function frame80() : *
      {
         stop();
      }
   }
}

